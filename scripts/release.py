#!/usr/bin/env python3
"""Package a stable release and sign both its archive and Sparkle feed."""
import hashlib
import plistlib
import re
import subprocess
from pathlib import Path
from xml.etree import ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
REPO = "https://github.com/amartinrojarvis/amstats"
SPARKLE = "http://www.andymatuschak.org/xml-namespaces/sparkle"
ET.register_namespace("sparkle", SPARKLE)


def prepare():
    app = ROOT / "dist/AMStats.app"
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    version = info["CFBundleShortVersionString"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("Release estable: usa una versión MAJOR.MINOR.PATCH")
    assert info["CFBundleVersion"] == version
    sign = ROOT / "target/sparkle/bin/sign_update"
    keys = ROOT / "target/sparkle/bin/generate_keys"
    account = ["--account", "dev.amartin.amstats"]
    public_key = subprocess.check_output([str(keys), *account, "-p"], text=True).strip()
    if public_key != info["SUPublicEDKey"]:
        raise ValueError("La clave del Llavero no coincide con la clave pública de AMStats")
    output = ROOT / "dist" / f"release-{version}"
    output.mkdir(exist_ok=True)
    archive = output / f"AMStats-{version}-arm64.zip"
    subprocess.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(app), str(archive)], check=True)
    signature = subprocess.check_output([str(sign), *account, "-p", str(archive)], text=True).strip()
    subprocess.run([str(sign), *account, "--verify", str(archive), signature], check=True)
    rss = ET.Element("rss", version="2.0")
    channel = ET.SubElement(rss, "channel")
    ET.SubElement(channel, "title").text = "AMStats"
    ET.SubElement(channel, "link").text = REPO
    ET.SubElement(channel, "description").text = "Versiones estables de AMStats"
    item = ET.SubElement(channel, "item")
    ET.SubElement(item, "title").text = f"AMStats {version}"
    ET.SubElement(item, "link").text = f"{REPO}/releases/tag/v{version}"
    ET.SubElement(item, f"{{{SPARKLE}}}version").text = version
    ET.SubElement(item, f"{{{SPARKLE}}}shortVersionString").text = version
    ET.SubElement(item, f"{{{SPARKLE}}}minimumSystemVersion").text = info["LSMinimumSystemVersion"]
    ET.SubElement(item, f"{{{SPARKLE}}}hardwareRequirements").text = "arm64"
    changelog = (ROOT / "CHANGELOG.md").read_text()
    notes = changelog.split(f"## {version} —", 1)[1].split("\n", 1)[1].split("\n## ", 1)[0].strip()
    ET.SubElement(item, "description").text = notes
    ET.SubElement(item, "enclosure", {
        "url": f"{REPO}/releases/download/v{version}/{archive.name}",
        "length": str(archive.stat().st_size),
        "type": "application/octet-stream",
        f"{{{SPARKLE}}}edSignature": signature,
    })
    ET.indent(rss)
    feed = output / "appcast.xml"
    ET.ElementTree(rss).write(feed, encoding="utf-8", xml_declaration=True)
    subprocess.run([str(sign), *account, str(feed)], check=True)
    subprocess.run([str(sign), *account, "--verify", str(feed)], check=True)
    checksums = output / "SHA256SUMS.txt"
    checksums.write_text("".join(f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n" for p in (archive, feed)))
    (output / "release-notes.md").write_text(
        f"AMStats {version} para macOS 13+ y Apple Silicon.\n\n{notes}\n\n"
        "Descarga el ZIP y copia AMStats.app a Aplicaciones. Si usas 0.1.0, esta primera instalación es manual; "
        "desde 0.2.0 puedes usar Buscar actualizaciones en el menú de AMStats; en 0.4.0 también está en Ajustes → Actualizaciones y en los paneles.\n\n"
        "Archivo y feed firmados con Ed25519. Aplicación con firma ad hoc, sin notarización de Apple.\n"
    )
    print(f"Release preparada y firmas verificadas: {output}")


if __name__ == "__main__":
    prepare()
