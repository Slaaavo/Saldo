"""Generate the eKasa receipt images the tests and the verify runs read.

Usage: <python with segno> -I make-receipts.py <worktree root>

It writes:
  src-tauri/tests/fixtures/ekasa/offline-receipt.png
  .claude/skills/verify-saldo/fixtures/receipts/offline-receipt.png
  .claude/skills/verify-saldo/fixtures/receipts/offline-receipt.pdf
  .claude/skills/verify-saldo/fixtures/receipts/not-ekasa.png
"""

import sys
from pathlib import Path

import segno

SPEC_CODE = "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23:237.23"
VERIFY_CODE = "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:261015120000:41:23.40"
VERIFY_LINES = [
    ("KAVA ZRNKOVA 1KG", "12.99", "23 %"),
    ("MLIEKO POLOTUCNE 1L", "1.29", "5 %"),
    ("PAPIER A4 500L", "6.62", "23 %"),
    ("PIVO SVETLE 0,5L", "2.50", "23 %"),
]


def receipt_pdf(code, lines, path):
    width, height = 226, 640
    matrix = [list(row) for row in segno.make(code, error="m").matrix]
    size = len(matrix)
    module = 170 / (size + 8)
    left = (width - module * (size + 8)) / 2 + module * 4
    top = 250

    ops = [
        "BT /F1 11 Tf 18 600 Td (VERIFY POTRAVINY s.r.o.) Tj ET",
        "BT /F1 8 Tf 18 586 Td (Hlavna 1, 811 01 Bratislava) Tj ET",
        "BT /F1 8 Tf 18 574 Td (DIC 2020000000  15.10.2026 12:00:00) Tj ET",
    ]
    y = 548
    for name, amount, rate in lines:
        ops.append(f"BT /F1 9 Tf 18 {y} Td ({name}) Tj ET")
        ops.append(f"BT /F1 9 Tf 150 {y} Td ({amount}) Tj ET")
        ops.append(f"BT /F1 9 Tf 186 {y} Td ({rate}) Tj ET")
        y -= 16
    total = f"{sum(float(amount) for _, amount, _ in lines):.2f}"
    ops.append(f"BT /F1 11 Tf 18 {y - 8} Td (SPOLU EUR) Tj ET")
    ops.append(f"BT /F1 11 Tf 150 {y - 8} Td ({total}) Tj ET")
    ops.append("BT /F1 8 Tf 18 274 Td (Offline doklad) Tj ET")
    ops.append("0 g")
    for r, row in enumerate(matrix):
        for c, dark in enumerate(row):
            if dark:
                ops.append(f"{left + c * module:.3f} {top - (r + 1) * module:.3f} {module:.3f} {module:.3f} re f")
    content = "\n".join(ops).encode("latin-1")

    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] "
        f"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".encode(),
        b"<< /Length " + str(len(content)).encode() + b" >>\nstream\n" + content + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for number, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{number} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode()
    for offset in offsets:
        out += f"{offset:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    Path(path).write_bytes(bytes(out))


def main():
    root = Path(sys.argv[1])
    unit = root / "src-tauri/tests/fixtures/ekasa"
    verify = root / ".claude/skills/verify-saldo/fixtures/receipts"
    unit.mkdir(parents=True, exist_ok=True)
    verify.mkdir(parents=True, exist_ok=True)
    segno.make(SPEC_CODE, error="m").save(str(unit / "offline-receipt.png"), scale=6, border=4)
    segno.make(VERIFY_CODE, error="m").save(str(verify / "offline-receipt.png"), scale=10, border=4)
    receipt_pdf(VERIFY_CODE, VERIFY_LINES, verify / "offline-receipt.pdf")
    segno.make("https://example.com/not-a-receipt", error="m").save(str(verify / "not-ekasa.png"), scale=10, border=4)


main()
