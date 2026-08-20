import pathlib, sys, pytest
sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "src"))
from protocol import Frame, encode_frame, decode_frame

def test_unicode_round_trip():
    frame = Frame("café", 2**64-1, "snowman ☃: ok")
    assert decode_frame(encode_frame(frame)) == frame

@pytest.mark.parametrize("bad", [b"01:a:1:0:", b"1:a:-1:0:", b"1:a:18446744073709551616:0:", b"1:a:1:0:x", b"1:\xff:1:0:"])
def test_rejects_bad_wire(bad):
    with pytest.raises(ValueError): decode_frame(bad)
