# Put the page and the side panel side by side at exactly 1280 x 800, a
# hairline between them, the way Chrome draws the panel beside a tab.
import sys
from PIL import Image
page_path, panel_path, out = sys.argv[1:4]
W, H = 1280, 800
page = Image.open(page_path).convert('RGB')
panel = Image.open(panel_path).convert('RGB')
pw = min(panel.width, 400)
panel = panel.crop((0, 0, pw, min(panel.height, H)))
if panel.height < H:
    canvas = Image.new('RGB', (pw, H), panel.getpixel((pw // 2, panel.height - 1)))
    canvas.paste(panel, (0, 0)); panel = canvas
left = W - pw - 1
if page.width != left or page.height != H:
    print('warning: page is', page.size, 'expected', (left, H))
page = page.crop((0, 0, min(page.width, left), min(page.height, H)))
img = Image.new('RGB', (W, H), (40, 40, 44))
img.paste(page, (0, 0))
img.paste(panel, (left + 1, 0))
img.save(out)
print(out, img.size, 'panel', panel.size, 'page', page.size)
