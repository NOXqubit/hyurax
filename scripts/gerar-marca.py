"""Desenha a marca Hyurax (H com travessa em subida) e gera ICO, PNG e RGBA 64."""
import math
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageChops

SAIDA = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "design" / "marca"
SAIDA.mkdir(parents=True, exist_ok=True)
S = 4096  # desenha grande, reduz no fim (sem serrilhado)
u = S / 64


def grad(c1, c2, angulo):
    v = Image.linear_gradient("L").resize((S, S))  # 0 em cima, 255 embaixo
    h = v.rotate(90)  # 255 à esquerda
    hd = ImageChops.invert(h)  # 255 à direita
    g = {90: v, 45: ImageChops.add(v, hd, scale=2), 135: ImageChops.add(ImageChops.invert(v), hd, scale=2)}[angulo]
    a = Image.new("RGB", (S, S), c1)
    b = Image.new("RGB", (S, S), c2)
    return Image.composite(b, a, g)


def mascara(desenho):
    m = Image.new("L", (S, S), 0)
    desenho(ImageDraw.Draw(m))
    return m


# fundo: quadrado arredondado, grafite azulado com luz no canto de cima
fundo_m = mascara(lambda d: d.rounded_rectangle([0, 0, S - 1, S - 1], radius=int(15 * u), fill=255))
fundo = grad((40, 46, 66), (8, 10, 18), 45)

# travessa: barra em subida, âmbar, passa por trás das colunas
def travessa(d):
    x1, y1, x2, y2, w = 18 * u, 39.5 * u, 46 * u, 24.5 * u, 8 * u
    ang = math.atan2(y2 - y1, x2 - x1)
    nx, ny = -math.sin(ang) * w / 2, math.cos(ang) * w / 2
    d.polygon([(x1 + nx, y1 + ny), (x2 + nx, y2 + ny), (x2 - nx, y2 - ny), (x1 - nx, y1 - ny)], fill=255)

bar_m = mascara(travessa)
bar = grad((236, 160, 40), (255, 222, 140), 135)

# colunas: cápsulas brancas, a da direita um pouco mais alta (subida)
def colunas(d):
    d.rounded_rectangle([15 * u, 15 * u, 24.5 * u, 50 * u], radius=int(4.75 * u), fill=255)
    d.rounded_rectangle([39.5 * u, 12 * u, 49 * u, 47 * u], radius=int(4.75 * u), fill=255)

col_m = mascara(colunas)
col = grad((255, 255, 255), (214, 220, 232), 90)

img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
img.paste(fundo, (0, 0), fundo_m)
img.paste(bar, (0, 0), ImageChops.multiply(bar_m, fundo_m))
img.paste(col, (0, 0), col_m)

grande = img.resize((1024, 1024), Image.LANCZOS)
grande.save(SAIDA / "hyurax-1024.png")
grande.resize((256, 256), Image.LANCZOS).save(SAIDA / "hyurax-256.png")
grande.save(SAIDA / "Hyurax.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
p64 = grande.resize((64, 64), Image.LANCZOS)
(SAIDA / "icone-64.rgba").write_bytes(p64.tobytes())
print("ok")
