from fractions import Fraction

def summarize(values):
    if not values: return None
    return {"count":len(values), "min":min(values), "max":max(values), "mean":Fraction(sum(values), len(values))}
