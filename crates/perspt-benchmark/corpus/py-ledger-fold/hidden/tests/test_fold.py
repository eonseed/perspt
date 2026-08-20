import pathlib, sys, pytest
sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "src"))
from ledger import fold_events

def test_valid_history():
    events = [{"type":"created","id":"x"},{"type":"added","amount":4},{"type":"added","amount":3},{"type":"closed"}]
    assert fold_events(events) == {"id":"x","total":7,"closed":True}

@pytest.mark.parametrize("events", [[{"type":"added","amount":1}], [{"type":"created","id":"x"},{"type":"created","id":"y"}], [{"type":"created","id":"x"},{"type":"closed"},{"type":"added","amount":1}], [{"type":"created","id":"x"},{"type":"added","amount":-1}], [{"type":"created","id":"x"},{"type":"added","amount":2**63}]])
def test_rejects_invalid(events):
    with pytest.raises(ValueError): fold_events(events)
