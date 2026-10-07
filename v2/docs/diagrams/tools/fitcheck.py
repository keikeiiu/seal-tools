"""Containment check for .drawio: every cell must fit inside its parent container.

PAGE-AWARE. An earlier version built one dict keyed by cell id across the WHOLE
file, so on a multi-page .drawio the second page's ids overwrote the first's --
and it reported overflows that were not there. A shop diagram was then reshaped
to satisfy that phantom, which is the worst thing a checker can do.
"""
import sys, xml.etree.ElementTree as ET

root = ET.parse(sys.argv[1]).getroot()
pages = list(root.iter('diagram')) or [root]
bad = 0

for page in pages:
    name = page.get('name') or '(single page)'
    cells = {}
    for c in page.iter('mxCell'):
        cid, g = c.get('id'), c.find('mxGeometry')
        if cid is None or g is None:
            continue
        cells[cid] = dict(parent=c.get('parent'),
                          x=float(g.get('x') or 0), y=float(g.get('y') or 0),
                          w=float(g.get('width') or 0), h=float(g.get('height') or 0))

    def absr(cid):
        c = cells[cid]; x, y, par = c['x'], c['y'], c['parent']
        while par in cells:
            p = cells[par]; x += p['x']; y += p['y']; par = p['parent']
        return x, y, c['w'], c['h']

    for cid, c in cells.items():
        par = c['parent']
        if par not in cells or par in ('0', '1'):
            continue
        px, py, pw, ph = absr(par)
        x, y, w, h = absr(cid)
        if x < px or y < py or x + w > px + pw or y + h > py + ph:
            bad += 1
            print(f"  [{name}] OVERFLOW {cid} inside {par}: "
                  f"cell y {y:.0f}..{y+h:.0f} vs parent y {py:.0f}..{py+ph:.0f} | "
                  f"x {x:.0f}..{x+w:.0f} vs {px:.0f}..{px+pw:.0f}")

print(f"pages: {len(pages)}   overflows: {bad}")
