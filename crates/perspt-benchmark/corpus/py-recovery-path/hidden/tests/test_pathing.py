import pathlib, sys, pytest
sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "src"))
from pathing import resolve_logical

def test_logical_contract():
    assert resolve_logical("/a/b", "../c/./") == "/a/c/"
    assert resolve_logical("/", "../../x") == "/x"
    assert resolve_logical("/a", "/z/../q") == "/q"
    assert resolve_logical("/", ".") == "/"

def test_nul_rejected():
    with pytest.raises(ValueError): resolve_logical("/a", "x\0y")
