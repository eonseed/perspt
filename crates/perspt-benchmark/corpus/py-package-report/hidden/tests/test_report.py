import pathlib, sys
root = pathlib.Path(__file__).parents[1]
sys.path[:0] = [str(root / "packages/core"), str(root / "packages/api")]
from core import summarize
from api import render
from fractions import Fraction

def test_core_exactness():
    assert summarize([]) is None
    assert summarize([10**30, 1, -10**30]) == {"count":3,"min":-10**30,"max":10**30,"mean":Fraction(1,3)}

def test_api_contract():
    assert render([]) == "null"
    assert render([1,2]) == '{"count":2,"max":2,"mean":"3/2","min":1}'
