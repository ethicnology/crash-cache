#!/usr/bin/env python3
"""Wire explicitly shared project dashboards using Metabase 0.63.18.2 click URLs."""
import json
import os
from urllib.parse import urlsplit

from provision_metabase import Api, COLLECTION, MARKER, ProvisionError, required, rows, unique_managed, observation_note, managed_note

VIEWS = {
    "overview": "Application health and installations",
    "diagnostics": "Diagnostic context and evidence",
    "pipeline": "Collection, logs and attachments",
    "details": "Error investigation",
    "tables": "Detailed verification tables",
}
LABELS = {"overview": "Overview", "diagnostics": "Diagnostic quality", "pipeline": "Collection", "details": "Investigate errors", "tables": "Data tables"}


def drill_url(destination, field, column=None):
    """Metabase substitutes and URL-encodes clicked columns and dashboard slugs."""
    return (destination + "?project={{project}}&from={{from}}&until={{until}}&"
            + field + "={{" + (column or field) + "}}")


def configure(api, base=""):
    if base and (urlsplit(base).scheme not in ("http", "https") or not urlsplit(base).netloc or urlsplit(base).username or urlsplit(base).path not in ("", "/") or urlsplit(base).query or urlsplit(base).fragment):
        raise ProvisionError("Public base must be an HTTP(S) origin or empty for same-origin links")
    collection = unique_managed(rows(api.call("GET", "/collection")), COLLECTION)
    if not collection:
        raise ProvisionError("Provision all five dashboard views before navigation")
    dashboards = rows(api.call("GET", "/dashboard"))
    selected = {key: unique_managed(dashboards, name, collection["id"]) for key, name in VIEWS.items()}
    if not all(selected.values()):
        raise ProvisionError("Provision all five dashboard views before navigation")
    destinations = {}
    for key, dashboard in selected.items():
        current = api.call("GET", f"/dashboard/{dashboard['id']}")
        public_id = current.get("public_uuid") or api.call("POST", f"/dashboard/{dashboard['id']}/public_link")["uuid"]
        destinations[key] = base.rstrip("/") + "/public/dashboard/" + public_id
    menu = " · ".join(f"[{LABELS[key]}]({url})" for key, url in destinations.items())
    for key, dashboard in selected.items():
        current = api.call("GET", f"/dashboard/{dashboard['id']}")
        tiles = []
        for tile in current["dashcards"]:
            visual = dict(tile.get("visualization_settings") or {})
            card = tile.get("card") or {}
            name = card.get("name", "")
            if tile.get("card_id") is None and managed_note(visual.get("text", "")):
                visual["text"] = observation_note(menu + "\n\n"
                    + ("Tap an error layer or device bar to investigate. Times UTC." if key == "overview"
                       else "Selected project data. Missing evidence is not proof of health. **All** clears detail filters. Times UTC."))
                tile["size_y"] = 4
            elif name in ("Error reports by capture layer", "Breadcrumb coverage by layer"):
                visual["click_behavior"] = {"type": "link", "linkType": "url", "linkTemplate": drill_url(destinations["details"], "layer")}
            elif name == "Observed devices and systems":
                visual["click_behavior"] = {"type": "link", "linkType": "url", "linkTemplate": drill_url(destinations["details"], "device_model")}
            elif name == "Platforms, systems and devices":
                settings = dict(visual.get("column_settings") or {})
                for field in ("app_version", "device_model"):
                    settings[json.dumps(["name", field], separators=(",", ":"))] = {"click_behavior": {
                        "type": "link", "linkType": "url", "linkTemplate": drill_url(destinations["details"], field)}}
                visual["column_settings"] = settings
            # The provisioner reserves three rows; navigation adds one more.
            row = tile["row"]
            if tile.get("card_id") and not (current.get("description") or "").endswith(" Navigation enabled."):
                row += 1
            tiles.append({**{name: tile[name] for name in ("id", "card_id", "col", "size_x", "size_y", "parameter_mappings")},
                          "row": row, "series": [], "visualization_settings": visual})
        api.call("PUT", f"/dashboard/{dashboard['id']}", {"dashcards": tiles,
            "description": MARKER + " Shared project summaries and filtered investigation. Navigation enabled."})
    return destinations


def main():
    if os.environ.get("METABASE_ENABLE_PUBLIC_NAVIGATION") != "1":
        raise ProvisionError("Set METABASE_ENABLE_PUBLIC_NAVIGATION=1 to explicitly publish these project dashboards")
    api = Api(required("METABASE_URL"))
    api.session = api.call("POST", "/session", {"username": required("METABASE_ADMIN_EMAIL"), "password": required("METABASE_ADMIN_PASSWORD")})["id"]
    try:
        print(json.dumps(configure(api, os.environ.get("METABASE_PUBLIC_BASE_URL", ""))))
    finally:
        api.call("DELETE", "/session", {"metabase-session-id": api.session})


if __name__ == "__main__":
    main()
