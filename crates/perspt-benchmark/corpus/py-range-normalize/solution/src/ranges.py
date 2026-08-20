def normalize_ranges(ranges):
    cleaned = []
    for start, end in ranges:
        if type(start) is not int or type(end) is not int: raise TypeError("integer endpoints required")
        if start > end: raise ValueError("reversed range")
        if start < end: cleaned.append((start, end))
    result = []
    for start, end in sorted(cleaned):
        if result and start <= result[-1][1]: result[-1] = (result[-1][0], max(result[-1][1], end))
        else: result.append((start, end))
    return result
