from .model import Frame

def _decimal(raw):
    if not raw or (len(raw) > 1 and raw.startswith(b"0")) or not raw.isdigit(): raise ValueError("noncanonical decimal")
    return int(raw)

def encode_frame(frame):
    if not 0 <= frame.sequence <= 2**64-1: raise ValueError("sequence")
    kind = frame.kind.encode(); payload = frame.payload.encode()
    return str(len(kind)).encode()+b":"+kind+b":"+str(frame.sequence).encode()+b":"+str(len(payload)).encode()+b":"+payload

def decode_frame(data):
    def take_number(rest):
        head, sep, tail = rest.partition(b":")
        if not sep: raise ValueError("missing separator")
        return _decimal(head), tail
    kind_len, rest = take_number(data)
    if len(rest) < kind_len + 1 or rest[kind_len:kind_len+1] != b":": raise ValueError("kind length")
    kind_raw, rest = rest[:kind_len], rest[kind_len+1:]
    sequence, rest = take_number(rest)
    if sequence > 2**64-1: raise ValueError("sequence")
    payload_len, rest = take_number(rest)
    if len(rest) != payload_len: raise ValueError("payload length")
    try: return Frame(kind_raw.decode(), sequence, rest.decode())
    except UnicodeDecodeError as error: raise ValueError("utf8") from error
