from dataclasses import dataclass

@dataclass(frozen=True)
class Frame:
    kind: str
    sequence: int
    payload: str
