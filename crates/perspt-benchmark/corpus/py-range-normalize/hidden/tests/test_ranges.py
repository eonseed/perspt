import pathlib, sys, pytest
sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "src"))
from ranges import normalize_ranges

def test_canonicalizes():
    assert normalize_ranges([(8,10),(1,3),(3,8),(5,5),(12,13)]) == [(1,10),(12,13)]

@pytest.mark.parametrize("bad", [[(2,1)], [(True,2)], [(1,2.0)]])
def test_rejects_invalid(bad):
    with pytest.raises((TypeError, ValueError)): normalize_ranges(bad)
