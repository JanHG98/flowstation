# Grenze zu ETSI ISI

`netcore-transit-v1` ist eine interne regionale Vermittlung und kein Ersatz für ETSI ISI. Die spätere ISI-Schicht wird als Adapter vor Transit gesetzt und übersetzt standardisierte Mobility-, Individual-Call-, Group-Call-, SDS- und Supplementary-Service-Informationen in das interne semantische Modell.

So bleibt NetCore-internes Routing stabil, während fremde SwMI-Anbindungen ihre eigenen Stage-3-PDUs, ROSE-/Transportprofile, Sicherheitsprofile und Interoperabilitätstests erhalten.
