#!/usr/bin/env python3
"""Servidor simulado SOLO para pruebas visuales en desarrollo (sin red).

Uso:  python3 tools/dev-mock/serve.py 8765
      BOUTECA_DEV_MOCK=http://127.0.0.1:8765 cargo run -p bouteca   (build debug)

Responde a las rutas de cada proveedor con datos sintéticos claramente falsos ("Demo …").
Las miniaturas son imágenes generadas (data URI), no contenido real.
"""
import base64, colorsys, json, random, struct, sys, zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

def png(w, h, hue):
    rows = []
    for y in range(h):
        row = bytearray([0])
        for x in range(w):
            r, g, b = colorsys.hsv_to_rgb((hue + x / w * 0.12 + y / h * 0.05) % 1, 0.55, 0.55 + 0.35 * (1 - y / h))
            row += bytes([int(r * 255), int(g * 255), int(b * 255)])
        rows.append(bytes(row))
    raw = zlib.compress(b"".join(rows), 9)
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", raw) + chunk(b"IEND", b"")
    return "data:image/png;base64," + base64.b64encode(data).decode()

WORDS = ["Demo gato bailando", "Demo reacción sorpresa", "Demo risa épica", "Demo perro confundido", "Demo aplausos", "Demo facepalm", "Demo baile clásico", "Demo susto", "Demo celebración", "Demo cara de póker"]

def wikimedia(q):
    random.seed(q.get("gsrsearch", [""])[0] + q.get("gsroffset", ["0"])[0])
    pages = []
    for i in range(18):
        kind = random.choice(["BITMAP", "VIDEO", "BITMAP", "AUDIO"])
        mime = {"BITMAP": random.choice(["image/gif", "image/jpeg"]), "VIDEO": "video/webm", "AUDIO": "audio/ogg"}[kind]
        pages.append({"pageid": random.randint(1, 10**8), "title": f"File:{random.choice(WORDS)} {i}.x", "index": i, "videoinfo": [{
            "url": f"https://upload.wikimedia.org/demo/{i}.{mime.split('/')[1]}", "descriptionurl": "https://commons.wikimedia.org/wiki/Main_Page",
            "thumburl": png(48, 32, random.random()), "width": random.choice([640, 1280, 1920]), "height": random.choice([360, 720, 1080]),
            "size": random.randint(10**5, 10**7), "mime": mime, "mediatype": kind, "duration": random.uniform(2, 60) if kind != "BITMAP" else None,
            "extmetadata": {"LicenseShortName": {"value": "CC BY-SA 4.0"}, "Artist": {"value": "Demo"}}, "derivatives": []}]})
    return {"continue": {"gsroffset": 18}, "query": {"pages": pages}}

def openverse(q):
    random.seed(q.get("q", [""])[0])
    return {"result_count": 12, "page_count": 1, "page": 1, "results": [{"id": f"ov{i}", "title": random.choice(WORDS), "creator": "Demo", "url": f"https://example.org/{i}.jpg",
        "thumbnail": png(48, 32, random.random()), "foreign_landing_url": "https://openverse.org", "license": "by", "license_version": "4.0", "source": "flickr",
        "tags": [{"name": "demo"}], "filetype": "jpg", "width": 1024, "height": 768} for i in range(12)]}

ROUTES = {"/w/api.php": wikimedia, "/v1/images/": openverse, "/v1/audio/": openverse}

class H(BaseHTTPRequestHandler):
    def do_GET(self):
        u = urlparse(self.path)
        f = ROUTES.get(u.path)
        body = json.dumps(f(parse_qs(u.query)) if f else {"response": {"numFound": 0, "docs": []}, "success": False}).encode()
        self.send_response(200 if f else 200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *a): pass

ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1]) if len(sys.argv) > 1 else 8765), H).serve_forever()
