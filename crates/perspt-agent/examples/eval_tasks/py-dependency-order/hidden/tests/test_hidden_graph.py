import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / 'src'))
from t.graph import topo_order


def test_cycles_return_none():
    assert topo_order([('a', 'b'), ('b', 'a')]) is None


def test_lexicographic_among_ready():
    assert topo_order([('z', 'm'), ('a', 'm')]) == ['a', 'z', 'm']
