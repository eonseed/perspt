import copy
from .model import Layer

def load_layers(layers: list[Layer]) -> Layer:
    def merge(base, later, path=""):
        for key, value in later.items():
            here = f"{path}.{key}" if path else key
            if key in base and isinstance(base[key], dict) and isinstance(value, dict): merge(base[key], value, here)
            elif key in base and type(base[key]) is not type(value): raise ValueError(f"type change at {here}")
            else: base[key] = copy.deepcopy(value)
        return base
    result = {}
    for layer in layers: merge(result, layer)
    return result
