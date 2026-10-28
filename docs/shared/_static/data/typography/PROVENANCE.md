# Local web typography

Typeset: Knuth-Plass boxes, glue, penalties and fitness classes.
https://github.com/bramstein/typeset/tree/48ebb5547de116db57e14454e19767e4ff9b2266
The linked list is unmodified. The line breaker has one change: after a break at a penalty, computeSum no longer adds the penalty's width (the hyphen) to the totals the next line starts from. Upstream, every line after a hyphen was measured one hyphen too narrow and could overflow. See TYPESET-LICENSE.txt.

Hypher: Frank Liang pattern-trie hyphenation.
https://github.com/bramstein/typeset/blob/48ebb5547de116db57e14454e19767e4ff9b2266/lib/hypher.js
See HYPHER-LICENSE.txt.

English: original TeX hyph-en-us.tex, Gerard D. C. Kuiken, version 2005-05-30.
https://github.com/hyphenation/tex-hyphen/tree/master/hyph-utf8/tex/generic/hyph-utf8/patterns/tex
The source and its permissive distribution notice are retained. english.js groups
patterns by their encoded length, replaces boundary dots with Hypher underscores,
and converts exception hyphens to U+2027. The reading edition uses three-character
left and right minima. No language service or CDN is used at runtime.
