/*global Typeset.LinkedList*/

Typeset.linebreak = (function () {

    /**
     * @preserve Knuth and Plass line breaking algorithm in JavaScript
     *
     * Licensed under the new BSD License.
     * Copyright 2009-2010, 2026 Bram Stein
     * All rights reserved.
     */
    const linebreak = function (nodes, lines, settings) {
        const options = {
            demerits: {
                line: settings?.demerits?.line ?? 10,
                flagged: settings?.demerits?.flagged ?? 100,
                fitness: settings?.demerits?.fitness ?? 3000
            },
            tolerance: settings?.tolerance ?? 2
        };
        const lineLengths = lines;
        const startTolerance = settings?.tolerance ?? 2;
        const maxTolerance = settings?.maxTolerance ?? startTolerance + 4;

        let activeNodes;
        let sum;
        let breaks;
        let tmp;

        function breakpoint(position, demerits, ratio, line, fitnessClass, totals, previous) {
            return {
                position,
                demerits,
                ratio,
                line,
                fitnessClass,
                totals: totals || {
                    width: 0,
                    stretch: 0,
                    shrink: 0
                },
                previous
            };
        }

        function isNonDiscardable(index) {
            const prev = nodes[index - 1];
            return prev.type === 'box' ||
                (prev.type === 'penalty' && prev.penalty !== linebreak.infinity);
        }

        function isFeasibleRatio(ratio, finalLine) {
            if (ratio < -1 || ratio === linebreak.infinity || !isFinite(ratio)) {
                return false;
            }
            if (finalLine) {
                return true;
            }
            return ratio <= options.tolerance;
        }

        function computeCost(start, end, active, currentLine) {
            let width = sum.width - active.totals.width;
            let stretch = 0;
            let shrink = 0;
            // If the current line index is within the list of linelengths, use it, otherwise use
            // the last line length of the list.
            const lineLength = currentLine < lineLengths.length ?
                lineLengths[currentLine - 1] :
                lineLengths[lineLengths.length - 1];

            if (nodes[end].type === 'penalty') {
                width += nodes[end].width;
            }

            if (width < lineLength) {
                stretch = sum.stretch - active.totals.stretch;

                if (stretch > 0) {
                    return (lineLength - width) / stretch;
                }
                return linebreak.infinity;

            }
            if (width > lineLength) {
                shrink = sum.shrink - active.totals.shrink;

                if (shrink > 0) {
                    return (lineLength - width) / shrink;
                }
                return -linebreak.infinity;
            }
            return 0;
        }

        // Record cumulative width, stretch and shrink after breaking at the given node.
        function computeSum(breakPointIndex) {
            const result = {
                width: sum.width,
                stretch: sum.stretch,
                shrink: sum.shrink
            };
            const node = nodes[breakPointIndex];

            if (node.type === 'glue') {
                result.width += node.width;
                result.stretch += node.stretch;
                result.shrink += node.shrink;
            }
            // CCM: a penalty's width (the hyphen) belongs to the line it ends. The running
            // sum never holds it, so adding it here measured the next line one hyphen too
            // narrow. Knuth and Plass (1981) discard the penalty at a break.
            return result;
        }

        // The main loop of the algorithm
        function mainLoop(node, index) {
            let active = activeNodes.first();
            let next = null;
            let ratio = 0;
            let demerits = 0;
            let candidates;
            let badness;
            let currentLine = 0;
            let tmpSum;
            let currentClass = 0;
            let candidate;
            let newNode;
            const finalLine = node.type === 'penalty' && node.penalty === -linebreak.infinity;

            // The inner loop iterates through all the active nodes with line < currentLine and then
            // breaks out to insert the new active node candidates before looking at the next active
            // nodes for the next lines. The result of this is that the active node list is always
            // sorted by line number.
            while (active !== null) {

                candidates = [
                    { demerits: Infinity },
                    { demerits: Infinity },
                    { demerits: Infinity },
                    { demerits: Infinity }
                ];

                // Iterate through the linked list of active nodes to find new potential active nodes
                // and deactivate current active nodes.
                while (active !== null) {
                    next = active.next;
                    currentLine = active.data.line + 1;
                    ratio = computeCost(active.data.position, index, active.data, currentLine);

                    // Deactivate nodes when the line is too overfull to shrink.
                    if (ratio < -1) {
                        activeNodes.remove(active);
                    }

                    // If the ratio is feasible, calculate the total demerits and record a
                    // candidate active node.
                    if (isFeasibleRatio(ratio, finalLine)) {
                        badness = 100 * Math.pow(Math.abs(ratio), 3);

                        // Positive penalty
                        if (node.type === 'penalty' && node.penalty >= 0) {
                            demerits = Math.pow(options.demerits.line + badness, 2) + Math.pow(node.penalty, 2);
                        // Negative penalty but not a forced break
                        } else if (node.type === 'penalty' && node.penalty !== -linebreak.infinity) {
                            demerits = Math.pow(options.demerits.line + badness, 2) - Math.pow(node.penalty, 2);
                        // All other cases
                        } else {
                            demerits = Math.pow(options.demerits.line + badness, 2);
                        }

                        if (node.type === 'penalty' && nodes[active.data.position].type === 'penalty') {
                            demerits += options.demerits.flagged * node.flagged * nodes[active.data.position].flagged;
                        }

                        // Calculate the fitness class for this candidate active node.
                        if (ratio < -0.5) {
                            currentClass = 0;
                        } else if (ratio <= 0.5) {
                            currentClass = 1;
                        } else if (ratio <= 1) {
                            currentClass = 2;
                        } else {
                            currentClass = 3;
                        }

                        // Add a fitness penalty to the demerits if the fitness classes of two adjacent lines
                        // differ too much.
                        if (Math.abs(currentClass - active.data.fitnessClass) > 1) {
                            demerits += options.demerits.fitness;
                        }

                        // Add the total demerits of the active node to get the total demerits of this candidate node.
                        demerits += active.data.demerits;

                        // Only store the best candidate for each fitness class
                        if (demerits < candidates[currentClass].demerits) {
                            candidates[currentClass] = {
                                active,
                                demerits,
                                ratio
                            };
                        }
                    }

                    active = next;

                    // Stop iterating through active nodes to insert new candidate active nodes in the active list
                    // before moving on to the active nodes for the next line.
                    // TODO: The Knuth and Plass paper suggests a conditional for currentLine < j0. This means paragraphs
                    // with identical line lengths will not be sorted by line number. Find out if that is a desirable outcome.
                    // For now I left this out, as it only adds minimal overhead to the algorithm and keeping the active node
                    // list sorted has a higher priority.
                    if (active !== null && active.data.line >= currentLine) {
                        break;
                    }
                }

                tmpSum = computeSum(index);

                for (let fitnessClass = 0; fitnessClass < candidates.length; fitnessClass += 1) {
                    candidate = candidates[fitnessClass];

                    if (candidate.demerits < Infinity) {
                        newNode = new Typeset.LinkedList.Node(breakpoint(
                            index,
                            candidate.demerits,
                            candidate.ratio,
                            candidate.active.data.line + 1,
                            fitnessClass,
                            tmpSum,
                            candidate.active
                        ));
                        if (active !== null) {
                            activeNodes.insertBefore(active, newNode);
                        } else {
                            activeNodes.push(newNode);
                        }
                    }
                }
            }
        }

        for (let tryTolerance = startTolerance; tryTolerance <= maxTolerance; tryTolerance += 1) {
            options.tolerance = tryTolerance;
            activeNodes = new Typeset.LinkedList();
            sum = {
                width: 0,
                stretch: 0,
                shrink: 0
            };
            breaks = [];
            tmp = {
                data: {
                    demerits: Infinity
                }
            };

            // Add an active node for the start of the paragraph.
            activeNodes.push(new Typeset.LinkedList.Node(breakpoint(0, 0, 0, 0, 0, undefined, null)));

            for (let index = 0; index < nodes.length; index += 1) {
                const node = nodes[index];

                if (node.type === 'box') {
                    sum.width += node.width;
                } else if (node.type === 'glue') {
                    if (index > 0 && isNonDiscardable(index)) {
                        mainLoop(node, index);
                    }
                    sum.width += node.width;
                    sum.stretch += node.stretch;
                    sum.shrink += node.shrink;
                } else if (node.type === 'penalty' && node.penalty !== linebreak.infinity) {
                    mainLoop(node, index);
                }
            }

            if (activeNodes.size() !== 0) {
                let maxPosition = 0;

                // Find the best active node at the furthest breakpoint (the end of the paragraph.)
                for (const activeNode of activeNodes) {
                    if (activeNode.data.position > maxPosition) {
                        maxPosition = activeNode.data.position;
                    }
                }

                for (const activeNode of activeNodes) {
                    if (activeNode.data.position === maxPosition && activeNode.data.demerits < tmp.data.demerits) {
                        tmp = activeNode;
                    }
                }

                while (tmp !== null) {
                    breaks.push({
                        position: tmp.data.position,
                        ratio: tmp.data.ratio
                    });
                    tmp = tmp.data.previous;
                }
                return breaks.reverse();
            }
        }

        return [];
    };

    linebreak.infinity = 10000;

    linebreak.glue = function (width, stretch, shrink) {
        return {
            type: 'glue',
            width,
            stretch,
            shrink
        };
    };

    linebreak.box = function (width, value) {
        return {
            type: 'box',
            width,
            value
        };
    };

    linebreak.penalty = function (width, penalty, flagged) {
        return {
            type: 'penalty',
            width,
            penalty,
            flagged
        };
    };

    return linebreak;

})();
