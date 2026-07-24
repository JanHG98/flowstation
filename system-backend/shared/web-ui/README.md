# NetCore Shared WebUI

Build-freie CSS- und ES-Modul-Bausteine für die Verwaltungsoberflächen der Backend-Dienste.
Die Assets benötigen weder Node.js noch einen zusätzlichen Frontend-Container in Produktion.

## Enthalten

- gemeinsames Layout, Karten, Tabellen, Formfelder und responsive Regeln,
- sichtbarer Open-Lab-Hinweis,
- Status-Badges,
- typisierter JSON-API-Client mit Timeout und Problem-Details-Fehlern,
- Bestätigungsdialoge und Toasts,
- deutsche und englische Basistexte,
- statische Demo unter `demo/index.html`.

## Einbindung

```html
<link rel="stylesheet" href="/assets/netcore.css">
<script type="module">
  import { NetCoreApiClient, statusBadge } from "/assets/netcore.js";
</script>
```

Die bestehenden Dienst-WebUIs bleiben eigenständig. Migration auf diese Bausteine erfolgt schrittweise und darf keine fachliche API oder Bedienfunktion entfernen.
