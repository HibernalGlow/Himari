"""Raw TCP sink: print the SNI carried by the very first TLS ClientHello on each connection.

No TLS library involved on purpose - this reads bytes off the wire so the measurement does not
depend on the same rustls stack the product under test uses.
"""

import socket
import struct
import sys
import threading


def parse_sni(buf: bytes) -> str:
    if len(buf) < 6 or buf[0] != 0x16:
        return f"<not-a-tls-record:{buf[:4].hex()}>"
    rec_len = struct.unpack("!H", buf[3:5])[0]
    hs = buf[5 : 5 + rec_len]
    if len(hs) < 4 or hs[0] != 0x01:
        return f"<not-a-client-hello:type={hs[0] if hs else 'empty'}>"
    p = 4  # skip handshake type(1) + length(3)
    p += 2  # client version
    p += 32  # random
    sid = hs[p]
    p += 1 + sid
    cs = struct.unpack("!H", hs[p : p + 2])[0]
    p += 2 + cs
    comp = hs[p]
    p += 1 + comp
    ext_end = struct.unpack("!H", hs[p : p + 2])[0]
    end = p + ext_end
    p += 2
    while p + 4 <= end:
        etype, elen = struct.unpack("!HH", hs[p : p + 4])
        body = hs[p + 4 : p + 4 + elen]
        if etype == 0x0000:  # server_name
            list_len = struct.unpack("!H", body[0:2])[0]
            name_type = body[2]
            nlen = struct.unpack("!H", body[3:5])[0]
            return f"{body[5:5+nlen].decode('ascii')}!name_type={name_type}"
        p += 4 + elen
    return "<hello-without-sni>"


def serve(listener: socket.socket) -> None:
    while True:
        try:
            tcp, peer = listener.accept()
        except OSError:
            return
        tcp.settimeout(5)
        buf = b""
        try:
            for _ in range(16):
                chunk = tcp.recv(4096)
                if not chunk:
                    break
                buf += chunk
                try:
                    print(f"SNI received: {parse_sni(buf)}", flush=True)
                    break
                except Exception:
                    continue
        except Exception as exc:  # a truncated hello is still a received hello
            print(f"SNI parse aborted after {len(buf)} bytes: {exc}", flush=True)
        finally:
            tcp.close()


def main() -> int:
    port = int(sys.argv[1])
    listener = socket.socket()
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind(("127.0.0.1", port))
    listener.listen(16)
    print(f"sink listening on 127.0.0.1:{port}", flush=True)
    serve(listener)
    return 0


if __name__ == "__main__":
    sys.exit(main())
