# Servidor local para desenvolver o site: serve a pasta site/ sem cache, para cada
# alteração aparecer no próximo recarregamento. Só escuta em 127.0.0.1.
#   python site/tools/servidor_dev.py [porta]
import functools
import http.server
import sys
from pathlib import Path

PASTA = Path(__file__).resolve().parents[1]


class SemCache(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".js": "text/javascript; charset=utf-8",
                      ".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8", ".svg": "image/svg+xml"}

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


if __name__ == "__main__":
    porta = int(sys.argv[1]) if len(sys.argv) > 1 else 8766
    manipulador = functools.partial(SemCache, directory=str(PASTA))
    with http.server.ThreadingHTTPServer(("127.0.0.1", porta), manipulador) as s:
        print(f"http://127.0.0.1:{porta}")
        s.serve_forever()
