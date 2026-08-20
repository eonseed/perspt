from .model import MAX_TOTAL

def fold_events(events):
    state = {"id": None, "total": 0, "closed": False}
    for event in events:
        if state["closed"]: raise ValueError("event after close")
        kind = event.get("type")
        if kind == "created" and state["id"] is None: state["id"] = event["id"]
        elif kind == "created": raise ValueError("duplicate create")
        elif kind == "added" and state["id"] is not None:
            amount = event["amount"]
            if not isinstance(amount, int) or amount < 0 or state["total"] + amount > MAX_TOTAL: raise ValueError("invalid amount")
            state["total"] += amount
        elif kind == "closed" and state["id"] is not None: state["closed"] = True
        else: raise ValueError("invalid transition")
    return dict(state)
