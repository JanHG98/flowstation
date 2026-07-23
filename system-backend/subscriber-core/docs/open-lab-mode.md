# Open-Lab-Modus

Der Subscriber Core besitzt in dieser Ausbaustufe keine Authentisierung und kein TLS. Die WebUI zeigt dies dauerhaft an. `security.mode` muss `open_lab` sein; andere Werte werden beim Start abgewiesen.

Der Container gehört ausschließlich in ein isoliertes Management-/Testnetz. Port 8100 darf nicht ins Internet oder in unvertrauenswürdige Netze veröffentlicht werden.
