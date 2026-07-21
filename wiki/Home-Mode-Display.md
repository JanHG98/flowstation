# Home-Mode-Display

Die Basisstation unterstützt die Rückmeldung eines Status an kompatible Endgeräte über Home-Mode-Display (HMD). Dabei wird nach einem U-STATUS eine passende Anzeigeantwort erzeugt.

## Ablauf

1. Endgerät sendet U-STATUS.
2. Basisstation ermittelt Statuscode und Directory-Label.
3. Dashboard und Statuscache werden aktualisiert.
4. Eine HMD-Antwort wird an das Endgerät gesendet.
5. Wiederholungen werden gedrosselt, um unnötigen SDS-Verkehr zu vermeiden.

## Status-Replay

Der zuletzt bekannte Status kann bei erneuter Registrierung erneut ausgesendet werden. Das ist besonders für Statusgruppen sinnvoll, deren Mitglieder zeitweise offline sind.

## Grenzen

- Darstellung hängt von Gerätefamilie und Programmierung ab.
- Ein korrekt bestätigter SDS-Transport garantiert nicht, dass das Gerät den Text in jeder Betriebsart sichtbar darstellt.
- Zu lange Labels oder Sonderzeichen können abgeschnitten oder anders dargestellt werden.

## Test

Mit jeder eingesetzten Gerätegeneration mindestens testen:

- normaler Status
- Statuswechsel in kurzer Folge
- Neustart/Re-Registrierung
- Offline-Mitglied einer Statusgruppe
- Notfallstatus und anschließende Rücknahme
