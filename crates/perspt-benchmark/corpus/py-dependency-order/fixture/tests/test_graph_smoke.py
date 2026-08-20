import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / 'src'))
from t.graph import topo_order


def test_orders_dependencies_first():
    assert topo_order([('a', 'b'), ('b', 'c')]) == ['a', 'b', 'c']
