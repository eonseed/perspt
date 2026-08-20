import pathlib, sys
sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "src"))
from service import lookup

def test_lookup_edges_and_marker():
    assert lookup("entry-000000") == 0
    assert lookup("release-channel") == 6427
    assert lookup("entry-008499") == 8499
    assert lookup("missing") is None
