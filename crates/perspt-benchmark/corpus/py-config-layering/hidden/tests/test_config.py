import pathlib, sys
sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "src"))
from app import load_layers

def test_recursive_copy_and_list_replace():
    a = {"db": {"host": "a", "port": 1}, "flags": ["old"]}
    b = {"db": {"port": 2}, "flags": ["new"]}
    assert load_layers([a, b]) == {"db": {"host": "a", "port": 2}, "flags": ["new"]}
    assert a["db"]["port"] == 1

def test_type_change_names_path():
    try: load_layers([{"db": {"port": 1}}, {"db": {"port": "x"}}])
    except ValueError as error: assert "db.port" in str(error)
    else: raise AssertionError("expected ValueError")
