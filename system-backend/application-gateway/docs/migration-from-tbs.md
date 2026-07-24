# Migration bestehender TBS-Integrationen

Bestehende lokale Integrationen werden in dieser Phase nicht blind entfernt. Die Migration erfolgt schrittweise:

1. Connector im Application Gateway im `shadow`-Modus anlegen.
2. Dasselbe Ereignis parallel an lokale Integration und Gateway spiegeln.
3. Payload, Zieladressierung, Rate Limit und Fehlerverhalten vergleichen.
4. Gateway auf `authoritative` setzen und lokale Integration nur noch als Fallback belassen.
5. Nach On-Air- und Ausfalltests die lokale Integration aus der TBS entfernen.

Lokale Schutzfunktionen, die bei Core-Ausfall weiter funktionieren müssen, bleiben an der TBS. Dazu gehören insbesondere unmittelbar funkkritische Notfall- oder Betriebssteuerungen. Externe Komfortintegrationen gehören dagegen in den Gateway-LXC.
