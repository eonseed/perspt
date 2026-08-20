import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / 'src'))
from t.lib import answer


def test_answer_is_int():
    assert isinstance(answer(), int)
