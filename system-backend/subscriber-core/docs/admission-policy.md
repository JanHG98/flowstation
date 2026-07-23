# Teilnehmer-Zugangsrichtlinie

Der Subscriber Core übersetzt seine Profile in eine versionierte TBS-Richtlinie.

```text
Subscriber Core
  -> Node Gateway /ws/backend
  -> TBS ControlCommand::SubscriberAccessPolicyApply
  -> MM Runtime-Override
```

`allow_list` überträgt alle aktivierten und zur Registrierung freigegebenen ISSIs. Eine leere Liste bedeutet dabei ausdrücklich deny-all. `open_network` setzt `allow_all = true`.

Jede TBS bestätigt Revision, Anzahl Einträge und Zahl der zur Re-Registrierung gezwungenen Teilnehmer. Der Status ist in der WebUI sichtbar.

## Lokale Dashboard-Whitelist

Die TBS-Dashboard-Whitelist bleibt als lokaler Notfall-/Testweg erhalten. Eine lokale Änderung überschreibt die zuletzt empfangene zentrale Richtlinie bis zur nächsten Synchronisation des Subscriber Core. Die WebUI zeigt deshalb pro TBS die tatsächlich bestätigte zentrale Revision an.
## Migrierte Teilnehmer

Die Richtlinie wird immer in Home-ISSIs ausgedrückt. Auf der TBS wird eine lokale VASSI vor jeder Zulassungs- oder Disconnect-Entscheidung wieder auf ihre Home-ISSI abgebildet. Diese Abbildung bleibt auch erhalten, nachdem die kurzlebige Migrationstransaktion aus der Diagnose-History entfernt wurde, und wird erst bei explizitem Detach oder Context-Transfer von der TBS gelöscht.

