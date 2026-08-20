import json, pathlib, sys
sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "core"))
from core import summarize

def render(values):
    summary = summarize(values)
    if summary is None: return "null"
    summary = dict(summary); mean = summary["mean"]; summary["mean"] = f"{mean.numerator}/{mean.denominator}"
    return json.dumps(summary, sort_keys=True, separators=(",", ":"))
