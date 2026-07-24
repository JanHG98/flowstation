# Open-Lab-Modus

Die KMF besitzt aktuell bewusst keine Tokens und keine Benutzerkonten. Damit sind alle erreichbaren Managementaktionen ungeschützt, einschließlich:

- Schlüsselgeneration,
- Rotation,
- Aktivierung,
- Widerruf und Zerstörung,
- Node-Bootstrap-Erzeugung,
- OTAR-Freigabe und Queueing,
- Backups.

Trotz fehlender Anmeldung gelten harte Secret-Grenzen:

- keine Rohschlüssel in WebUI oder Management-API,
- keine Rohschlüssel in Logs, Audit, Metrics oder Export,
- kein Bootstrap-Geheimnis in API-Antworten,
- OTAR nur als nodegebundener Envelope,
- Vault und Bootstrap-Dateien mit Modus 0600.

Vor Produktivbetrieb sind mindestens erforderlich:

- TLS und mTLS für Edge-Zugriffe,
- zentrale Anmeldung und RBAC,
- echte Vier-Augen-Identitäten,
- HSM/PKCS#11,
- signierte Audit-Exporte,
- gesicherte Key Ceremony,
- getrennte Break-Glass-Verfahren.
