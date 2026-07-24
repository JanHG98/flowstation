# Lab-Provider und Geheimnisbehandlung

`lab_hmac_sha256` erzeugt aus einem lokalen 32-Byte-Seed deterministische Teilnehmerprüfwerte. Der Seed wird beim ersten Start mit Modus `0600` angelegt.

Nicht über WebUI oder normale API ausgegeben werden:

- Seed
- abgeleitete Teilnehmergeheimnisse
- Challenge
- erwartete Antwort
- DCK

Angezeigt werden nur verkürzte SHA-256-Fingerprints und Referenzen. Das Edge-Protokoll darf Challenge beziehungsweise DCK einmalig für die technische Übergabe an die TBS ausliefern. Diese Ausnahme ist sichtbar konfiguriert und wird in der KMF-Stufe durch einen abgesicherten Providerkanal ersetzt.
