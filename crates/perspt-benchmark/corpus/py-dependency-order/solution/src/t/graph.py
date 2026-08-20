import heapq


def topo_order(edges):
    successors = {}
    indegree = {}
    for before, after in edges:
        successors.setdefault(before, set()).add(after)
        successors.setdefault(after, set())
        indegree.setdefault(before, 0)
        indegree.setdefault(after, 0)
    for before, values in successors.items():
        for after in values:
            indegree[after] += 1
    ready = [node for node, degree in indegree.items() if degree == 0]
    heapq.heapify(ready)
    result = []
    while ready:
        node = heapq.heappop(ready)
        result.append(node)
        for after in sorted(successors[node]):
            indegree[after] -= 1
            if indegree[after] == 0:
                heapq.heappush(ready, after)
    return result if len(result) == len(indegree) else None
