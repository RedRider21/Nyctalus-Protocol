// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
package org.nyctalus.mobile;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.SystemClock;
import android.provider.MediaStore;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.JavascriptInterface;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.widget.Toast;

import java.io.File;
import java.io.IOException;
import java.net.HttpURLConnection;
import java.net.URL;
import java.security.SecureRandom;
import java.util.ArrayList;

/**
 * App Nyctalus per Android: un guscio leggero attorno alla web-app della chat.
 *
 * Come Termux-NexusSEC-OS (da cui deriva): il motore e il piccolo servizio
 * (nyctalus-mobile) girano DENTRO Termux; questa app chiede a Termux di avviarli
 * (intent RUN_COMMAND) e mostra la web-app su http://127.0.0.1:18731.
 *
 * Sicurezza: su Android qualunque app può collegarsi a 127.0.0.1, quindi il servizio
 * accetta solo chi presenta un segreto. L'app ne genera uno (casuale, 128 bit), lo
 * scrive in ~/.nyctalus-mobile/token tramite Termux e apre la pagina con ?t=SEGRETO;
 * il servizio lo trasforma in un cookie. Le foto scattate dalla chat restano nella
 * cartella privata dell'app (FotoProvider), non nella galleria.
 *
 * Nessuna dipendenza AndroidX: solo il framework Android.
 */
public class MainActivity extends Activity {

    private static final int PORTA = 18731;
    private static final String URL_BASE = "http://127.0.0.1:" + PORTA;
    private static final String TERMUX = "com.termux";
    private static final String PERMESSO_TERMUX = "com.termux.permission.RUN_COMMAND";
    private static final String PREFS = "nyctalus";
    private static final int RICHIESTA_PERMESSO = 1, RICHIESTA_FILE = 2;
    private static final long ATTESA_MAX_MS = 40000;

    private static final String GITHUB = "https://github.com/RedRider21/Nyctalus-Protocol";

    // colori della schermata d'avvio: quelli del tema scelto nella web-app (ponte tema())
    private int C_SFONDO, C_ACCENTO, C_TENUE, C_SCHEDA, C_BORDO, C_TESTO, C_SU_ACCENTO = 0xFFFFFFFF;

    private WebView web;
    private TextView stato;
    private String segreto;
    private boolean caricata = false, pronto = false, entraAppenaPronto = false, inPausa = false;
    private View pulsanteEntra;      // la casella «Chat»: si accende quando Nyctalus è pronto
    private int portaSocksBrowser = 18733;
    private static final String SITO = "https://redrider21.github.io/Nyctalus-Protocol/";
    private volatile boolean avviando = false, provatoInPrimoPiano = false;

    private ValueCallback<Uri[]> attesaFile;
    private Uri scattoInCorso;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        caricaTema();
        getWindow().setStatusBarColor(C_SFONDO);
        getWindow().setNavigationBarColor(C_SFONDO);
        segreto = segreto();
        pulisciScattiVecchi();
        setContentView(costruisciAvvio());
        if (Build.VERSION.SDK_INT >= 23 && checkSelfPermission(PERMESSO_TERMUX) != PackageManager.PERMISSION_GRANTED
                && termuxInstallato()) {
            requestPermissions(new String[]{PERMESSO_TERMUX}, RICHIESTA_PERMESSO);   // poi avvia()
        } else {
            new Thread(this::avvia).start();
        }
    }

    @Override
    public void onRequestPermissionsResult(int richiesta, String[] permessi, int[] esiti) {
        super.onRequestPermissionsResult(richiesta, permessi, esiti);
        if (richiesta == RICHIESTA_PERMESSO) new Thread(this::avvia).start();
    }

    /** Stessi colori dei temi della web-app (Notte, Ardesia, Viola, Chiaro). */
    private void caricaTema() {
        switch (getSharedPreferences(PREFS, MODE_PRIVATE).getString("tema", "notte")) {
            case "ardesia":
                C_SFONDO = 0xFF0E1621; C_ACCENTO = 0xFF6AB3F3; C_TENUE = 0xFF7F91A4;
                C_SCHEDA = 0xFF17212B; C_BORDO = 0xFF22303D; C_TESTO = 0xFFF5F5F5; break;
            case "viola":
                C_SFONDO = 0xFF120F1C; C_ACCENTO = 0xFF9D85FF; C_TENUE = 0xFF8A82A6;
                C_SCHEDA = 0xFF1A1626; C_BORDO = 0xFF2A2440; C_TESTO = 0xFFF1EDFF; break;
            case "chiaro":
                C_SFONDO = 0xFFEEF1F6; C_ACCENTO = 0xFF2F6FE0; C_TENUE = 0xFF7D889C;
                C_SCHEDA = 0xFFFFFFFF; C_BORDO = 0xFFDDE3EC; C_TESTO = 0xFF1A2233; break;
            default:
                C_SFONDO = 0xFF0A0E1A; C_ACCENTO = 0xFF5AA9FF; C_TENUE = 0xFF6B7797;
                C_SCHEDA = 0xFF0E1424; C_BORDO = 0xFF1B2740; C_TESTO = 0xFFE6ECFF; break;
        }
    }

    /** Segreto d'accesso dell'app: creato una volta, conservato nelle preferenze private. */
    private String segreto() {
        SharedPreferences p = getSharedPreferences(PREFS, MODE_PRIVATE);
        String s = p.getString("segreto", null);
        if (s == null || s.length() < 32) {
            byte[] b = new byte[16];
            new SecureRandom().nextBytes(b);
            StringBuilder sb = new StringBuilder();
            for (byte x : b) sb.append(String.format("%02x", x));
            s = sb.toString();
            p.edit().putString("segreto", s).apply();
        }
        return s;
    }

    /**
     * Il comando che Termux esegue: scrive il segreto (permessi 600) e avvia il servizio
     * in sottofondo. Tocca solo ~/.nyctalus-mobile e ~/nyctalus: niente di altri progetti.
     */
    private String comandoAvvio() {
        return "mkdir -p ~/.nyctalus-mobile && chmod 700 ~/.nyctalus-mobile && "
             + "rm -f ~/.nyctalus-mobile/token && (umask 077; printf %s " + segreto + " > ~/.nyctalus-mobile/token) && "
             + "(command -v nyctalus-mobile >/dev/null 2>&1 && nyctalus-mobile --sfondo "
             + "|| { [ -x ~/nyctalus/nyctalus-mobile ] && bash ~/nyctalus/nyctalus-mobile --sfondo; } "
             + "|| { echo 'Nyctalus non è ancora installato in Termux: nell app tocca «Installa / aggiorna Nyctalus».'; false; })";
    }

    // ------------------------------------------------------------- AVVIO -----
    /**
     * Splash: logo grande al centro (compare in dissolvenza e poi «respira» piano), nome,
     * stato e le caselle delle funzioni, come il mini menu del PC: Chat, Browser, File
     * (in arrivo), Info e manuali. Si entra solo toccandole. Gli strumenti di Termux
     * stanno raccolti sotto «Strumenti ▾».
     */
    private View costruisciAvvio() {
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.VERTICAL);
        r.setGravity(Gravity.CENTER_HORIZONTAL);
        GradientDrawable fondo = new GradientDrawable(GradientDrawable.Orientation.TOP_BOTTOM,
                new int[]{C_SCHEDA, C_SFONDO, C_SFONDO});
        r.setBackground(fondo);
        r.setPadding(dp(22), dp(22), dp(22), dp(16));
        r.addView(spazio());

        // alone luminoso dietro al logo
        android.widget.FrameLayout cornice = new android.widget.FrameLayout(this);
        LinearLayout.LayoutParams cp = new LinearLayout.LayoutParams(dp(230), dp(230));
        cornice.setLayoutParams(cp);
        View alone = new View(this);
        GradientDrawable g = new GradientDrawable();
        g.setGradientType(GradientDrawable.RADIAL_GRADIENT);
        g.setGradientRadius(dp(115));
        g.setColors(new int[]{(C_ACCENTO & 0x00FFFFFF) | 0x55000000, (C_ACCENTO & 0x00FFFFFF) | 0x00000000});
        alone.setBackground(g);
        cornice.addView(alone, new android.widget.FrameLayout.LayoutParams(dp(230), dp(230)));
        ImageView logo = new ImageView(this);
        logo.setImageResource(R.drawable.logo);
        android.widget.FrameLayout.LayoutParams lp = new android.widget.FrameLayout.LayoutParams(dp(170), dp(170));
        lp.gravity = Gravity.CENTER;
        cornice.addView(logo, lp);
        r.addView(cornice);

        TextView t = testo("Nyctalus", C_TESTO, 34);
        t.setTypeface(Typeface.create("sans-serif-light", Typeface.NORMAL));
        t.setLetterSpacing(0.08f);
        r.addView(t);
        TextView sub = testo("Anonymous Protocol", C_ACCENTO, 13);
        sub.setLetterSpacing(0.18f);
        sub.setPadding(0, dp(4), 0, 0);
        r.addView(sub);

        stato = testo("avvio in corso…", C_TENUE, 13);
        LinearLayout.LayoutParams sp = avvolgi();
        sp.topMargin = dp(26);
        stato.setLayoutParams(sp);
        r.addView(stato);

        // le funzioni, come il mini menu del PC: una fila di pulsanti piatti, tutti uguali
        LinearLayout griglia = riga();
        ((LinearLayout.LayoutParams) griglia.getLayoutParams()).topMargin = dp(20);
        pulsanteEntra = casella("💬", "Chat", v -> entra());
        pulsanteEntra.setAlpha(pronto ? 1f : 0.55f);
        griglia.addView(pulsanteEntra);
        griglia.addView(casella("🌐", "Browser", v -> sceltaBrowser()));
        View file = casella("📁", "File", v -> avviso(
                "Il gestore dei file (invio e ricezione attraverso Nyctalus) è in preparazione."));
        file.setAlpha(0.45f);
        griglia.addView(file);
        griglia.addView(casella("ℹ️", "Info", v -> apriBrowser(SITO, false)));
        r.addView(griglia);
        r.addView(spazio());

        // strumenti (come il pannello di Termux-NexusSEC-OS), raccolti sotto «Strumenti ▾»
        LinearLayout strumenti = new LinearLayout(this);
        strumenti.setOrientation(LinearLayout.VERTICAL);
        strumenti.setLayoutParams(new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT));
        LinearLayout riga1 = riga(), riga2 = riga(), riga3 = riga(), riga4 = riga();
        riga1.addView(cella("⬇  Installa Termux", v -> installaTermux()));
        riga1.addView(cella("↻  Aggiorna Termux", v -> aggiornaTermux()));
        riga2.addView(cella("🦇  Installa / aggiorna Nyctalus in Termux", v -> installaNyctalus()));
        riga3.addView(cella("⌨  Apri Termux", v -> apriTermux()));
        riga3.addView(cella("📋  Comando di avvio", v -> copiaComando()));
        Button gh = cella("☁  Aggiornamenti da GitHub (presto)", v -> aggiornamentiGithub());
        gh.setAlpha(0.45f);
        riga4.addView(gh);
        for (LinearLayout x : new LinearLayout[]{riga1, riga2, riga3, riga4}) {
            ((LinearLayout.LayoutParams) x.getLayoutParams()).topMargin = dp(8);
            strumenti.addView(x);
        }
        boolean aperti = getSharedPreferences(PREFS, MODE_PRIVATE).getBoolean("strumenti", false);
        strumenti.setVisibility(aperti ? View.VISIBLE : View.GONE);
        TextView apri = testo(aperti ? "Strumenti ▴" : "Strumenti ▾", C_TENUE, 14);
        apri.setPadding(dp(20), dp(10), dp(20), dp(4));
        apri.setOnClickListener(v -> {
            boolean ora = strumenti.getVisibility() != View.VISIBLE;
            strumenti.setVisibility(ora ? View.VISIBLE : View.GONE);
            apri.setText(ora ? "Strumenti ▴" : "Strumenti ▾");
            getSharedPreferences(PREFS, MODE_PRIVATE).edit().putBoolean("strumenti", ora).apply();
        });
        r.addView(apri);
        r.addView(strumenti);
        TextView versione = testo("versione " + versioneApp(), C_TENUE, 11);
        versione.setAlpha(0.7f);
        versione.setPadding(0, dp(10), 0, 0);
        r.addView(versione);

        // animazioni: il logo compare e poi «respira»; il resto sale in dissolvenza
        logo.setAlpha(0f);
        logo.setScaleX(0.82f);
        logo.setScaleY(0.82f);
        alone.setAlpha(0f);
        logo.animate().alpha(1f).scaleX(1f).scaleY(1f).setDuration(900)
                .setInterpolator(new android.view.animation.DecelerateInterpolator()).withEndAction(() -> {
                    android.animation.ObjectAnimator a = android.animation.ObjectAnimator.ofPropertyValuesHolder(logo,
                            android.animation.PropertyValuesHolder.ofFloat(View.SCALE_X, 1f, 1.04f),
                            android.animation.PropertyValuesHolder.ofFloat(View.SCALE_Y, 1f, 1.04f));
                    a.setDuration(2200);
                    a.setRepeatCount(android.animation.ValueAnimator.INFINITE);
                    a.setRepeatMode(android.animation.ValueAnimator.REVERSE);
                    a.start();
                }).start();
        alone.animate().alpha(1f).setDuration(1400).start();
        logo.setOnClickListener(v -> apriBrowser(SITO, false));     // come il logo del mini menu del PC
        for (View v : new View[]{t, sub, stato, griglia}) {
            v.setTranslationY(dp(14));
            float fine = v.getAlpha();
            v.setAlpha(0f);
            v.animate().alpha(fine).translationY(0).setStartDelay(350).setDuration(700).start();
        }
        return r;
    }

    /**
     * 1) se il servizio risponde già, entra; 2) chiede a Termux di avviarlo in
     * sottofondo; 3) se non basta (Termux non ancora in memoria), apre Termux un
     * istante e poi torna qui da sola.
     */
    private void avvia() {
        avviando = true;
        try {
            if (!termuxInstallato()) {
                imposta("Termux non è installato.\nTocca «Installa Termux», poi installa Nyctalus dentro Termux.");
                return;
            }
            // Si avvia sempre il comando: scrive il segreto di QUESTA app anche se il
            // servizio è già acceso (lo rilegge da solo), poi si attende la risposta.
            imposta("avvio di Nyctalus in Termux…");
            avviaInTermux();
            if (attendi(8000)) { servizioPronto(); return; }
            if (!provatoInPrimoPiano) {
                provatoInPrimoPiano = true;
                imposta("apro Termux un istante per avviare Nyctalus…");
                apriTermux();
                avviaInTermux();
                if (attendi(ATTESA_MAX_MS)) { riportaDavanti(); servizioPronto(); return; }
            } else if (attendi(ATTESA_MAX_MS)) { servizioPronto(); return; }
            imposta("Nyctalus non risponde.\n"
                  + "Se Nyctalus non è ancora installato in Termux tocca «Installa / aggiorna Nyctalus».\n"
                  + "Altrimenti: «Comando di avvio», incollalo in Termux e torna qui.");
        } finally {
            avviando = false;
        }
    }

    private void avviaInTermux() {
        try {
            Intent i = new Intent();
            i.setComponent(new ComponentName(TERMUX, "com.termux.app.RunCommandService"));
            i.setAction("com.termux.RUN_COMMAND");
            i.putExtra("com.termux.RUN_COMMAND_PATH", "/data/data/com.termux/files/usr/bin/bash");
            i.putExtra("com.termux.RUN_COMMAND_ARGUMENTS", new String[]{"-lc", comandoAvvio()});
            i.putExtra("com.termux.RUN_COMMAND_WORKDIR", "/data/data/com.termux/files/home");
            i.putExtra("com.termux.RUN_COMMAND_BACKGROUND", true);
            i.putExtra("com.termux.RUN_COMMAND_SESSION_ACTION", "0");
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) startForegroundService(i);
            else startService(i);
        } catch (Exception ignorata) {
            // permesso negato o «allow-external-apps» assente: resta il comando da incollare
        }
    }

    private boolean attendi(long ms) {
        long fine = SystemClock.elapsedRealtime() + ms;
        while (SystemClock.elapsedRealtime() < fine) {
            if (risponde()) return true;
            dormi(900);
        }
        return false;
    }

    /** True se il servizio risponde con il NOSTRO segreto (non un altro programma sulla porta). */
    private boolean risponde() {
        HttpURLConnection c = null;
        try {
            c = (HttpURLConnection) new URL(URL_BASE + "/api/stato").openConnection();
            c.setConnectTimeout(800);
            c.setReadTimeout(1500);
            c.setRequestProperty("Cookie", "nyct=" + segreto);
            c.setInstanceFollowRedirects(false);
            if (c.getResponseCode() != 200) return false;
            try (java.io.InputStream in = c.getInputStream()) {
                java.io.ByteArrayOutputStream b = new java.io.ByteArrayOutputStream();
                byte[] buf = new byte[4096];
                int n;
                while ((n = in.read(buf)) > 0) b.write(buf, 0, n);
                portaSocksBrowser = new org.json.JSONObject(b.toString("UTF-8")).optInt("socks_browser", portaSocksBrowser);
            } catch (Exception ignorata) { }
            return true;
        } catch (IOException e) {
            return false;
        } finally {
            if (c != null) c.disconnect();
        }
    }

    private void servizioPronto() {
        pronto = true;
        imposta("● Nyctalus è pronto — scegli cosa aprire");
        runOnUiThread(() -> { if (pulsanteEntra != null) pulsanteEntra.animate().alpha(1f).setDuration(300).start(); });
        if (entraAppenaPronto) mostraWeb();
    }

    private void entra() {
        if (pronto) { mostraWeb(); return; }
        entraAppenaPronto = true;
        imposta("avvio… entro appena Nyctalus è pronto");
        if (!avviando) new Thread(this::avvia).start();
    }

    @Override
    protected void onPause() {
        super.onPause();
        inPausa = true;
    }

    /** Tornando nell'app (es. da Termux) si ricontrolla il servizio per qualche secondo. */
    @Override
    protected void onResume() {
        super.onResume();
        mostraErroreBrowser();
        if (inPausa && !caricata && !avviando) {
            inPausa = false;
            new Thread(() -> { if (attendi(12000)) servizioPronto(); }).start();
        }
    }

    private boolean termuxInstallato() {
        return getPackageManager().getLaunchIntentForPackage(TERMUX) != null;
    }

    private void riportaDavanti() {
        runOnUiThread(() -> {
            try {
                Intent i = new Intent(this, MainActivity.class);
                i.addFlags(Intent.FLAG_ACTIVITY_REORDER_TO_FRONT | Intent.FLAG_ACTIVITY_SINGLE_TOP);
                startActivity(i);
            } catch (Exception ignorata) { }
        });
    }

    private void apriTermux() {
        Intent i = getPackageManager().getLaunchIntentForPackage(TERMUX);
        if (i != null) startActivity(i);
        else avviso("Termux non è installato: installalo da F-Droid.");
    }

    /** Termux non sta sul Play Store (versione vecchia e ferma): F-Droid o GitHub. */
    private void installaTermux() {
        scegli(termuxInstallato() ? "Termux è già installato. Reinstallarlo da:" : "Scarica e installa Termux da:",
                new String[]{"F-Droid (consigliato)", "GitHub (file …arm64-v8a.apk)"},
                k -> apriPagina(k == 0 ? "https://f-droid.org/packages/com.termux/"
                                      : "https://github.com/termux/termux-app/releases/latest"),
                "Dopo l'installazione apri Termux una volta e aspetta che finisca di prepararsi.");
    }

    private void aggiornaTermux() {
        scegli("Aggiorna Termux", new String[]{
                "L'app Termux (pagina F-Droid)",
                "I pacchetti di Termux (pkg upgrade)"}, k -> {
            if (k == 0) { apriPagina("https://f-droid.org/packages/com.termux/"); return; }
            conferma("Aggiornare tutti i pacchetti di Termux?",
                    "«pkg upgrade» aggiorna i pacchetti di TUTTO Termux, anche quelli usati da altri "
                  + "progetti. Di solito va bene, ma se un altro progetto dipende da una versione "
                  + "precisa, meglio farlo a mano. L'aggiornamento si vede in Termux.",
                    () -> eseguiInTermux("yes | pkg upgrade -y; echo; echo 'Aggiornamento finito: puoi tornare nell app Nyctalus.'",
                            false, "Aggiornamento dei pacchetti in corso in Termux."));
        }, null);
    }

    /**
     * Installa (o aggiorna) Nyctalus in Termux col pacchetto che l'app porta con sé:
     * l'app lo serve su 127.0.0.1 per qualche minuto, Termux lo scarica, ne verifica
     * l'impronta SHA-256, lancia l'installatore (che convive con gli altri progetti),
     * avvia il servizio e riporta qui. L'installazione si vede in Termux.
     */
    private void installaNyctalus() {
        if (!termuxInstallato()) { installaTermux(); return; }
        if (!ServitorePacchetto.presente(this)) {
            avviso("Questa versione dell'app non contiene il pacchetto: usa nyctalus-termux.tar.gz (vedi manuale).");
            return;
        }
        ServitorePacchetto sp = avviaServitore();
        if (sp == null) return;
        provatoInPrimoPiano = true;
        eseguiInTermux(comandoInstallazione(sp), true, null);
        imposta("Installazione in corso in Termux…\nAlla fine l'app torna qui: poi tocca Entra.");
    }

    private ServitorePacchetto avviaServitore() {
        try {
            ServitorePacchetto vecchio = ServizioPacchetto.attivo;
            if (vecchio != null) vecchio.interrupt();
            ServitorePacchetto sp = new ServitorePacchetto(this);
            sp.start();
            ServizioPacchetto.attivo = sp;
            Intent i = new Intent(this, ServizioPacchetto.class);
            if (Build.VERSION.SDK_INT >= 26) startForegroundService(i);
            else startService(i);
            return sp;
        } catch (Exception e) {
            avviso("Impossibile preparare il pacchetto: " + e.getMessage());
            return null;
        }
    }

    /** Scarica dall'app il pacchetto, ne verifica l'impronta, installa, avvia e riapre l'app. */
    private String comandoInstallazione(ServitorePacchetto sp) {
        String cmd = "D=$(mktemp -d) && cd \"$D\" && "
                + "{ command -v curl >/dev/null 2>&1 || pkg install -y curl; } && "
                + "echo '» Scarico Nyctalus dall app…' && "
                + "curl -fsS --retry 5 --retry-connrefused --retry-delay 1 --connect-timeout 5 --max-time 300 "
                + sp.indirizzo() + " -o n.tar.gz && "
                + "echo '" + sp.impronta + "  n.tar.gz' | sha256sum -c --quiet && "
                + "tar xzf n.tar.gz && bash nyctalus-termux/installa.sh && cd ~ && rm -rf \"$D\" && "
                + comandoAvvio() + " && "
                + "echo && echo '✓ Fatto: torno nell app Nyctalus…' && sleep 2 && "
                + "(am start -n org.nyctalus.mobile/.MainActivity >/dev/null 2>&1 || true)"
                + " || { echo; echo '✗ Installazione non riuscita: leggi i messaggi qui sopra.'; cd ~; rm -rf \"$D\"; }";
        return cmd;
    }

    /** Un pulsante del menu: piatto, tutti dello stesso colore, icona sopra e nome sotto. */
    private View casella(String icona, String nome, View.OnClickListener cl) {
        LinearLayout c = new LinearLayout(this);
        c.setOrientation(LinearLayout.VERTICAL);
        c.setGravity(Gravity.CENTER);
        c.setPadding(dp(4), dp(10), dp(4), dp(8));
        GradientDrawable g = new GradientDrawable();
        g.setCornerRadius(dp(14));
        g.setColor(C_SCHEDA);
        g.setStroke(dp(1), C_BORDO);
        GradientDrawable maschera = new GradientDrawable();
        maschera.setCornerRadius(dp(14));
        maschera.setColor(0xFFFFFFFF);
        c.setBackground(new android.graphics.drawable.RippleDrawable(
                android.content.res.ColorStateList.valueOf((C_ACCENTO & 0x00FFFFFF) | 0x40000000), g, maschera));
        TextView i = testo(icona, C_TESTO, 20);
        TextView n = testo(nome, C_TENUE, 11);
        n.setLetterSpacing(0.04f);
        n.setPadding(0, dp(4), 0, 0);
        c.addView(i);
        c.addView(n);
        c.setClickable(true);
        c.setOnClickListener(cl);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        lp.setMargins(dp(4), 0, dp(4), 0);
        c.setLayoutParams(lp);
        return c;
    }

    /** Se il browser si è chiuso per un errore, lo si dice (con il testo da copiare e riferire). */
    private void mostraErroreBrowser() {
        java.io.File f = new java.io.File(getFilesDir(), "errore-browser.txt");
        if (!f.exists()) return;
        String testo;
        try {
            byte[] b = new byte[(int) Math.min(f.length(), 65536)];
            try (java.io.FileInputStream in = new java.io.FileInputStream(f)) {
                int n = 0, letti;
                while (n < b.length && (letti = in.read(b, n, b.length - n)) > 0) n += letti;
                testo = new String(b, 0, n, "UTF-8");
            }
        } catch (Exception e) {
            testo = "(non leggibile)";
        }
        f.delete();
        final String t = testo.length() > 4000 ? testo.substring(0, 4000) : testo;
        new android.app.AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Il browser si è chiuso per un errore")
                .setMessage(t)
                .setPositiveButton("Copia il testo", (d, k) -> { copiaNegliAppunti(t); avviso("Copiato: mandalo a chi sviluppa"); })
                .setNegativeButton("Chiudi", null).show();
    }

    /** Browser: normale, o in modalità Nyctalus (tutto il traffico nella rete). */
    private void sceltaBrowser() {
        new android.app.AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Browser")
                .setItems(new String[]{
                        "🦇  Modalità Nyctalus — anonimo, siti .nyct" + (pronto ? "" : " (avvia prima Nyctalus)"),
                        "🌐  Navigazione normale"},
                        (d, k) -> apriBrowser(null, k == 0))
                .setNegativeButton("Annulla", null).show();
    }

    /** Il browser (derivato da quello di NovaOS) gira in un processo suo: vedi BrowserActivity. */
    private void apriBrowser(String url, boolean nyctalus) {
        Intent i = new Intent(this, BrowserActivity.class);
        if (url != null) i.putExtra("url", url);
        i.putExtra("nyctalus", nyctalus);
        i.putExtra("porta_socks", portaSocksBrowser);
        startActivity(i);
    }

    /** Gli aggiornamenti dal repository arriveranno quando il progetto sarà pubblicato. */
    private void aggiornamentiGithub() {
        avviso("In arrivo: gli aggiornamenti da GitHub si attivano quando il progetto sarà pubblicato. "
             + "Per ora aggiorna installando la nuova APK e poi «Installa / aggiorna Nyctalus».");
    }

    /**
     * Esegue un comando in una sessione VISIBILE di Termux (si vede l'avanzamento).
     * Lo copia anche negli appunti: se Termux non lo esegue (la prima volta manca
     * «allow-external-apps», che l'installatore di Nyctalus abilita), basta incollarlo.
     */
    private void eseguiInTermux(String cmd, boolean spiega, String messaggio) {
        copiaNegliAppunti(cmd);
        try {
            Intent i = new Intent();
            i.setComponent(new ComponentName(TERMUX, "com.termux.app.RunCommandService"));
            i.setAction("com.termux.RUN_COMMAND");
            i.putExtra("com.termux.RUN_COMMAND_PATH", "/data/data/com.termux/files/usr/bin/bash");
            i.putExtra("com.termux.RUN_COMMAND_ARGUMENTS", new String[]{"-lc", cmd});
            i.putExtra("com.termux.RUN_COMMAND_WORKDIR", "/data/data/com.termux/files/home");
            i.putExtra("com.termux.RUN_COMMAND_BACKGROUND", false);
            i.putExtra("com.termux.RUN_COMMAND_SESSION_ACTION", "0");   // nuova sessione, in primo piano
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) startForegroundService(i);
            else startService(i);
        } catch (Exception ignorata) { }
        apriTermux();
        if (spiega) {
            avviso("Se in Termux non parte nulla (succede solo la prima volta): tieni premuto → Incolla → Invio. "
                 + "Il comando è già negli appunti.");
        } else if (messaggio != null) {
            avviso(messaggio);
        }
    }

    private void apriPagina(String url) {
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)));
        } catch (Exception e) {
            avviso("Apri: " + url);
        }
    }

    private String versioneApp() {
        try {
            return getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
        } catch (Exception e) {
            return "?";
        }
    }

    private interface Scelta { void fatta(int k); }

    private void scegli(String titolo, String[] voci, Scelta s, String nota) {
        android.app.AlertDialog.Builder b = new android.app.AlertDialog.Builder(this,
                android.R.style.Theme_DeviceDefault_Dialog_Alert);
        b.setTitle(titolo).setItems(voci, (d, k) -> s.fatta(k)).setNegativeButton("Annulla", null);
        android.app.AlertDialog d = b.create();
        d.show();
        if (nota != null) avviso(nota);
    }

    private void conferma(String titolo, String testo, Runnable si) {
        new android.app.AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle(titolo).setMessage(testo)
                .setPositiveButton("Procedi", (d, k) -> si.run())
                .setNegativeButton("Annulla", null).show();
    }

    /** Ripiego sempre valido: il comando negli appunti, da incollare in Termux. */
    /**
     * Ripiego sempre valido: un comando da incollare in Termux che AVVIA Nyctalus se è
     * installato e altrimenti lo INSTALLA prima (col pacchetto dell'app), poi lo avvia.
     */
    private void copiaComando() {
        String cmd = comandoAvvio() + " && echo '✓ Nyctalus avviato: torna nell app Nyctalus.'";
        ServitorePacchetto sp = ServitorePacchetto.presente(this) ? avviaServitore() : null;
        if (sp != null) {
            cmd = "if [ -x ~/nyctalus/nyctalus-mobile ]; then " + cmd + "; else " + comandoInstallazione(sp) + "; fi";
        }
        copiaNegliAppunti(cmd);
        apriTermux();
        avviso("Comando copiato. In Termux: tieni premuto → Incolla → Invio. "
             + "Se Nyctalus manca lo installa, poi lo avvia (vale 15 minuti).");
    }

    // ----------------------------------------------------------- WEB-APP -----
    /** Tasto indietro dalla chat (alla sua prima schermata): si torna alla splash. */
    private void tornaAllAvvio() {
        setContentView(costruisciAvvio());
        if (pronto) imposta("● Nyctalus è pronto — scegli cosa aprire");
    }

    private void mostraWeb() {
        if (web != null) { runOnUiThread(() -> setContentView(web)); return; }   // già pronta: si rientra
        if (caricata) return;
        caricata = true;
        runOnUiThread(() -> {
            web = new WebView(this);
            web.setLayoutParams(new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT));
            WebSettings s = web.getSettings();
            s.setJavaScriptEnabled(true);
            s.setDomStorageEnabled(true);           // localStorage e IndexedDB (cache della chat)
            s.setMediaPlaybackRequiresUserGesture(true);
            s.setAllowFileAccess(false);
            s.setAllowContentAccess(true);          // per leggere gli scatti da FotoProvider
            web.addJavascriptInterface(new Ponte(), "NyctalusHost");
            web.setWebChromeClient(new Sceglitore());
            web.setWebViewClient(new WebViewClient() {
                @Override
                public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest req) {
                    Uri u = req.getUrl();
                    if ("127.0.0.1".equals(u.getHost()) && u.getPort() == PORTA) return false;
                    // tutto il resto fuori dall'app, e solo se l'utente lo ha toccato
                    if (req.hasGesture()) {
                        try { startActivity(new Intent(Intent.ACTION_VIEW, u)); } catch (Exception ignorata) { }
                    }
                    return true;
                }

                @Override
                public void onReceivedError(WebView v, WebResourceRequest req, WebResourceError err) {
                    if (req != null && req.isForMainFrame()) {
                        WebView vecchia = web;
                        web = null;
                        if (vecchia != null) vecchia.post(vecchia::destroy);
                        caricata = false;
                        pronto = false;
                        entraAppenaPronto = false;
                        setContentView(costruisciAvvio());
                        imposta("Collegamento con Nyctalus perso.\nTocca «Entra» per riavviarlo.");
                    }
                }
            });
            setContentView(web);
            web.loadUrl(URL_BASE + "/?t=" + segreto);
        });
    }

    /** Collega gli <input type="file"> della web-app a galleria, file e fotocamera. */
    private class Sceglitore extends WebChromeClient {
        @Override
        public boolean onShowFileChooser(WebView v, ValueCallback<Uri[]> cb, FileChooserParams p) {
            if (attesaFile != null) attesaFile.onReceiveValue(null);
            attesaFile = cb;
            scattoInCorso = null;
            String tipi = android.text.TextUtils.join(",", p.getAcceptTypes());
            Intent i;
            if (p.isCaptureEnabled() && (tipi.contains("image") || tipi.contains("video"))) {
                boolean video = tipi.contains("video") && !tipi.contains("image");
                String nome = (video ? "VID_" : "IMG_") + System.currentTimeMillis() + (video ? ".mp4" : ".jpg");
                scattoInCorso = FotoProvider.indirizzo(nome);
                i = new Intent(video ? MediaStore.ACTION_VIDEO_CAPTURE : MediaStore.ACTION_IMAGE_CAPTURE);
                i.putExtra(MediaStore.EXTRA_OUTPUT, scattoInCorso);
                i.setClipData(ClipData.newRawUri("", scattoInCorso));
                i.addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION | Intent.FLAG_GRANT_READ_URI_PERMISSION);
            } else {
                i = p.createIntent();
                if (p.getMode() == FileChooserParams.MODE_OPEN_MULTIPLE) i.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
            }
            try {
                startActivityForResult(i, RICHIESTA_FILE);
                return true;
            } catch (Exception e) {
                attesaFile = null;
                avviso("Nessuna app disponibile per questa scelta.");
                return false;
            }
        }
    }

    @Override
    protected void onActivityResult(int richiesta, int esito, Intent dati) {
        super.onActivityResult(richiesta, esito, dati);
        if (richiesta != RICHIESTA_FILE || attesaFile == null) return;
        Uri[] scelti = null;
        if (esito == RESULT_OK) {
            if (scattoInCorso != null) {
                File f = new File(FotoProvider.cartella(this), scattoInCorso.getLastPathSegment());
                if (f.length() > 0) scelti = new Uri[]{scattoInCorso};
                else if (dati != null && dati.getData() != null) scelti = new Uri[]{dati.getData()};  // fotocamere che ignorano EXTRA_OUTPUT
            } else if (dati != null) {
                ArrayList<Uri> l = new ArrayList<>();
                ClipData cd = dati.getClipData();
                if (cd != null) for (int k = 0; k < cd.getItemCount(); k++) l.add(cd.getItemAt(k).getUri());
                else if (dati.getData() != null) l.add(dati.getData());
                if (!l.isEmpty()) scelti = l.toArray(new Uri[0]);
            }
        }
        attesaFile.onReceiveValue(scelti);
        attesaFile = null;
        scattoInCorso = null;
    }

    /** Gli scatti servono solo il tempo dell'invio: quelli di più di un giorno si cancellano. */
    private void pulisciScattiVecchi() {
        File[] l = FotoProvider.cartella(this).listFiles();
        if (l == null) return;
        long limite = System.currentTimeMillis() - 24L * 3600 * 1000;
        for (File f : l) if (f.lastModified() < limite) f.delete();
    }

    /** Funzioni esposte alla web-app come window.NyctalusHost (solo dentro l'APK). */
    private class Ponte {
        @JavascriptInterface
        public void esci() {
            runOnUiThread(() -> finishAndRemoveTask());
        }

        /** La web-app comunica il tema scelto: la schermata d'avvio userà gli stessi colori. */
        @JavascriptInterface
        public void tema(String id) {
            if (id != null && id.matches("[a-z]{1,20}"))
                getSharedPreferences(PREFS, MODE_PRIVATE).edit().putString("tema", id).apply();
        }

        @JavascriptInterface
        public void apriBrowser(String url, boolean nyctalus) {
            if (url == null || !(url.startsWith("http://") || url.startsWith("https://"))) return;
            runOnUiThread(() -> MainActivity.this.apriBrowser(url, nyctalus));
        }

        @JavascriptInterface
        public void copia(String t) {
            if (t != null) copiaNegliAppunti(t);
        }

        @JavascriptInterface
        public void condividi(String t) {
            if (t == null) return;
            runOnUiThread(() -> {
                Intent i = new Intent(Intent.ACTION_SEND);
                i.setType("text/plain");
                i.putExtra(Intent.EXTRA_TEXT, t);
                try { startActivity(Intent.createChooser(i, "Condividi")); } catch (Exception ignorata) { }
            });
        }
    }

    // ------------------------------------------------------------ UTILITÀ ----
    private void copiaNegliAppunti(String t) {
        try {
            ClipboardManager cb = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
            if (cb != null) cb.setPrimaryClip(ClipData.newPlainText("Nyctalus", t));
        } catch (Exception ignorata) { }
    }

    private TextView testo(String s, int colore, int sp) {
        TextView t = new TextView(this);
        t.setText(s);
        t.setTextColor(colore);
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, sp);
        t.setGravity(Gravity.CENTER);
        return t;
    }

    private Button bottone(String s, boolean primario, View.OnClickListener cl) {
        Button b = new Button(this);
        b.setText(s);
        b.setAllCaps(false);
        b.setOnClickListener(cl);
        b.setTextSize(TypedValue.COMPLEX_UNIT_SP, primario ? 17 : 13);
        b.setTextColor(primario ? C_SU_ACCENTO : C_ACCENTO);
        b.setMinHeight(0);
        b.setMinimumHeight(0);
        b.setPadding(dp(primario ? 30 : 12), dp(primario ? 13 : 11), dp(primario ? 30 : 12), dp(primario ? 13 : 11));
        GradientDrawable g = new GradientDrawable();
        g.setCornerRadius(dp(primario ? 24 : 12));
        if (primario) g.setColor(C_ACCENTO);
        else { g.setColor(C_SCHEDA); g.setStroke(dp(1), C_BORDO); }
        b.setBackground(g);
        return b;
    }

    private LinearLayout riga() {
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.HORIZONTAL);
        r.setLayoutParams(new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT));
        return r;
    }

    private Button cella(String s, View.OnClickListener cl) {
        Button b = bottone(s, false, cl);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        lp.setMargins(dp(4), 0, dp(4), 0);
        b.setLayoutParams(lp);
        return b;
    }

    private View spazio() {
        View v = new View(this);
        v.setLayoutParams(new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        return v;
    }

    private LinearLayout.LayoutParams avvolgi() {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
    }

    private void imposta(String t) {
        runOnUiThread(() -> { if (stato != null) stato.setText(t); });
    }

    private void avviso(String t) {
        runOnUiThread(() -> Toast.makeText(this, t, Toast.LENGTH_LONG).show());
    }

    private int dp(int v) {
        return (int) TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, v, getResources().getDisplayMetrics());
    }

    private void dormi(long ms) {
        try { Thread.sleep(ms); } catch (InterruptedException ignorata) { }
    }

    @Override
    public boolean onKeyDown(int tasto, KeyEvent e) {
        if (tasto == KeyEvent.KEYCODE_BACK && web != null && web.getParent() != null) {
            if (web.canGoBack()) web.goBack();      // dentro la chat (es. da una conversazione all'elenco)
            else tornaAllAvvio();                   // dall'elenco: di nuovo alla schermata di partenza
            return true;
        }
        return super.onKeyDown(tasto, e);
    }
}
