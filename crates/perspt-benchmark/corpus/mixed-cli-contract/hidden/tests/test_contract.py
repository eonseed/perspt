import pathlib, sys, pytest
sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "python"))
from client import report, ReportError

def test_cross_language_report():
    assert report([4, -2, 9]) == {"count":3,"min":-2,"max":9,"sum":11}
    assert report([]) == {"count":0,"min":None,"max":None,"sum":0}

def test_bad_input_is_reported():
    with pytest.raises(ReportError): report(["not-an-int"])
