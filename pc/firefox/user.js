// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
//
// Nyctalus · profilo Firefox blindato (PIANO-ANONIMATO, punto J, fase 1).
// Lo scrive `avvia-firefox.sh` a ogni avvio (@SOCKS_PORT@ = la porta della rete Nyctalus):
// le modifiche a mano qui dentro si perdono. Modello: le scelte di Tor Browser.

// --- tutto attraverso Nyctalus, anche i nomi dei siti ---------------------------------------
user_pref("network.proxy.type", 1);
user_pref("network.proxy.socks", "127.0.0.1");
user_pref("network.proxy.socks_port", @SOCKS_PORT@);
user_pref("network.proxy.socks_version", 5);
user_pref("network.proxy.socks_remote_dns", true);          // il DNS lo fa l'uscita, non il PC
user_pref("network.proxy.no_proxies_on", "");
user_pref("network.proxy.allow_hijacking_localhost", true); // anche «localhost» passa dal proxy
user_pref("network.proxy.failover_direct", false);          // se il proxy manca: niente, mai diretto
user_pref("network.trr.mode", 5);                           // niente DNS-over-HTTPS fuori dalla rete
user_pref("network.dns.disablePrefetch", true);
user_pref("network.dns.disableIPv6", true);
user_pref("network.prefetch-next", false);
user_pref("network.predictor.enabled", false);
user_pref("network.http.speculative-parallel-limit", 0);
user_pref("browser.urlbar.speculativeConnect.enabled", false);
user_pref("browser.places.speculativeConnect.enabled", false);
user_pref("network.captive-portal-service.enabled", false);   // niente «controlli di rete» verso Mozilla
user_pref("network.connectivity-service.enabled", false);
user_pref("browser.fixup.domainsuffixwhitelist.nyct", true);  // «xxx.nyct» è un sito, non una ricerca
user_pref("dom.security.https_only_mode", false);              // i .nyct sono già cifrati da Nyctalus…
user_pref("dom.security.https_first", true);                   // …per gli altri si prova prima HTTPS

// --- WebRTC e sensori: possono rivelare l'IP o il dispositivo --------------------------------
user_pref("media.peerconnection.enabled", false);
user_pref("media.navigator.enabled", false);
user_pref("geo.enabled", false);
user_pref("dom.battery.enabled", false);
user_pref("dom.gamepad.enabled", false);
user_pref("device.sensors.enabled", false);
user_pref("beacon.enabled", false);

// --- impronta uguale per tutti (come Tor Browser) -------------------------------------------
user_pref("privacy.resistFingerprinting", true);
user_pref("privacy.resistFingerprinting.letterboxing", true);  // finestra a misure fisse
user_pref("privacy.firstparty.isolate", true);                 // ogni sito nel suo recinto
user_pref("webgl.disabled", true);
user_pref("javascript.options.wasm", true);
user_pref("intl.accept_languages", "en-US, en");
user_pref("javascript.use_us_english_locale", true);

// --- niente tracce sul disco ----------------------------------------------------------------
user_pref("browser.privatebrowsing.autostart", true);
user_pref("browser.cache.disk.enable", false);
user_pref("browser.formfill.enable", false);
user_pref("signon.rememberSignons", false);
user_pref("places.history.enabled", false);
user_pref("browser.sessionstore.resume_from_crash", false);
user_pref("network.http.referer.XOriginPolicy", 2);
user_pref("network.http.referer.XOriginTrimmingPolicy", 2);

// --- niente collegamenti a Mozilla, Google o altri in sottofondo ----------------------------
user_pref("toolkit.telemetry.enabled", false);
user_pref("toolkit.telemetry.unified", false);
user_pref("toolkit.telemetry.archive.enabled", false);
user_pref("datareporting.healthreport.uploadEnabled", false);
user_pref("datareporting.policy.dataSubmissionEnabled", false);
user_pref("app.shield.optoutstudies.enabled", false);
user_pref("app.normandy.enabled", false);
user_pref("browser.discovery.enabled", false);
user_pref("browser.safebrowsing.malware.enabled", false);     // chiederebbe a Google
user_pref("browser.safebrowsing.phishing.enabled", false);
user_pref("browser.safebrowsing.downloads.enabled", false);
user_pref("browser.safebrowsing.downloads.remote.enabled", false);
user_pref("extensions.pocket.enabled", false);
user_pref("extensions.getAddons.cache.enabled", false);
user_pref("browser.search.suggest.enabled", false);
user_pref("browser.urlbar.suggest.searches", false);
user_pref("browser.newtabpage.enabled", false);
user_pref("browser.newtabpage.activity-stream.feeds.telemetry", false);
user_pref("browser.newtabpage.activity-stream.telemetry", false);
user_pref("browser.newtabpage.activity-stream.feeds.topsites", false);
user_pref("browser.newtabpage.activity-stream.showSponsored", false);
user_pref("browser.newtabpage.activity-stream.showSponsoredTopSites", false);
user_pref("browser.ping-centre.telemetry", false);
user_pref("app.update.auto", false);                           // gli aggiornamenti li fa il sistema
user_pref("extensions.update.enabled", false);
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("browser.aboutwelcome.enabled", false);
user_pref("trailhead.firstrun.didSeeAboutWelcome", true);
user_pref("datareporting.policy.firstRunURL", "");
user_pref("browser.startup.homepage_override.mstone", "ignore");

// --- avvio ----------------------------------------------------------------------------------
user_pref("browser.startup.page", 1);
user_pref("browser.startup.homepage", "@PAGINA@");
