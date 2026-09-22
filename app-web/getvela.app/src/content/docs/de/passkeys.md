---
title: So funktionieren Passkeys
description: "Was ein Passkey ist, wo der private Schlüssel bei jeder Schlüsselart liegt, warum es kein Geheimnis gibt, das man dir abfischen kann, und wovor ein Passkey dich nicht schützt."
source: b23999b2ed69
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# So funktionieren Passkeys

Die Schlüssel, die eine Vela-Wallet steuern, sind **Passkeys**:
WebAuthn-Anmeldedaten auf der Kurve P-256. Dein Gerät oder Sicherheitsschlüssel
erzeugt jeden davon, verwahrt den privaten Schlüssel und benutzt ihn erst, wenn du mit
Face ID, Fingerabdruck, Geräte-PIN oder Berührung und PIN am Sicherheitsschlüssel
bestätigst. Vela erhält den privaten Schlüssel nie; synchronisiert ihn ein
Passwortmanager, verwahrt der Manager ihn verschlüsselt für dich.

## Was ein Passkey ist

Ein Passkey ist ein Schlüsselpaar aus öffentlichem und privatem Schlüssel, erzeugt für
eine einzige Website – bei Vela `getvela.app`. Eine App erhält den privaten Schlüssel
nie; sie kann den Authentifikator nur bitten, etwas zu signieren, und der fragt vorher
dich.

Wo der private Schlüssel liegt, hängt von der Schlüsselart ab:

| Schlüsselart | Wo der private Schlüssel liegt | Auf andere Geräte synchronisiert? |
| --- | --- | --- |
| **Dieses Gerät** – Face ID, Touch ID, Fingerabdruck, Windows Hello | Im Passwortmanager deiner Plattform (iCloud-Schlüsselbund, Google Passwortmanager) oder in einem Passwortmanager wie 1Password | Meist ja, Ende-zu-Ende-verschlüsselt, wenn die Synchronisierung an ist. Windows-Hello-Schlüssel bleiben auf dem PC |
| **Ein anderes Handy**, verbunden per QR-Code | Im Passwortmanager dieses Handys | Wie oben |
| **Ein Hardware-Sicherheitsschlüssel** (YubiKey und andere FIDO2-Schlüssel, per USB oder NFC) | Im Sicherheitsschlüssel selbst | Nie |

Eine Vela-Wallet kann bis zu sieben Schlüssel in beliebiger Mischung nutzen, gewählt
beim Erstellen; [Signaturschlüssel und Sicherheitsschlüssel](/de/docs/signers)
behandelt diese Wahl.

## Kein Geheimnis zum Abfischen

Phishing funktioniert, indem man dich dazu bringt, ein Geheimnis herauszugeben. Eine
Seed-Phrase sind zwölf Wörter, die man dir abschwatzen kann, bis du sie irgendwo
eintippst. Ein Passkey hat **kein Geheimnis, das du eintippen könntest**: Es gibt
nichts zu verraten, nichts einzufügen, und eine gefälschte Seite kann nicht danach
fragen. Und weil ein Passkey für eine einzige Website erzeugt wird, bietet dein
Browser einen `getvela.app`-Passkey nur Seiten auf getvela.app und ihren Subdomains
an.

Damit fällt eine ganze Klasse von Verlusten weg, die bei der Selbstverwahrung häufig
ist: die gestohlene Wiederherstellungsphrase.

## Wovor ein Passkey dich nicht schützt

<Callout type="warning" title="Ein Passkey signiert, was du freigibst">
Die Abfrage deines Handys oder Browsers sagt, <em>welcher</em> Schlüssel benutzt wird,
nicht, <em>was</em> signiert wird. Ein Passkey signiert eine schädliche Transaktion
genauso bereitwillig wie eine gute, wenn du sie freigibst. Deshalb dekodiert Vela jede
Transaktion, bevor du signierst (<a href="/de/docs/clear-signing">Klartext-Signatur</a>),
und deshalb kommt es auf die Seite an, die sie anzeigt
(<a href="/de/docs/bybit-attack">der Bybit-Angriff</a>).
</Callout>

Er schützt auch nicht vor jemandem, der dein entsperrtes Handy hat und dessen Prüfung
bestehen kann, oder der das Konto kontrolliert, über das dein Passkey synchronisiert
wird. Richte einen Gerätecode ein, sichere dein Apple- oder Google-Konto und zieh
einen Hardware-Sicherheitsschlüssel in Betracht, der nirgends synchronisiert wird.

## Wie sich das Signieren anfühlt

1. Du bestätigst eine Transaktion in Vela, nachdem du gelesen hast, was sie tut.
2. Dein Gerät oder Sicherheitsschlüssel verlangt Face ID, einen Fingerabdruck, deine
   PIN oder Berührung plus PIN.
3. Es signiert, und nur die Signatur geht zurück an die App.
4. Die App übergibt die signierte Operation an das Relay, das sie einreicht; dein
   Wallet-Vertrag prüft die Passkey-Signatur on-chain, bevor er irgendetwas tut.

## Wohin der öffentliche Schlüssel geht

Die **öffentlichen** Hälften deiner Schlüssel werden in einem öffentlichen Register auf
der Gnosis Chain festgehalten, damit ein neues Gerät deine Wallet finden kann. Darum
geht es in [Wiederherstellung und Anmeldung](/de/docs/recovery).

Weiter: [Signaturschlüssel und Sicherheitsschlüssel](/de/docs/signers).
