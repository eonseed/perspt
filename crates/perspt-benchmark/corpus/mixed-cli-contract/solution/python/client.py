import json, pathlib, subprocess

class ReportError(RuntimeError):
    pass

def report(values):
    root = pathlib.Path(__file__).parents[1]
    process = subprocess.run(["cargo", "run", "--quiet", "--", *map(str, values)], cwd=root, text=True, capture_output=True)
    if process.returncode: raise ReportError(process.stderr.strip())
    return json.loads(process.stdout)
