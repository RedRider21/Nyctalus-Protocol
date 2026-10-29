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

    // colori del tema «Notte» della web-app
    private static final int C_SFONDO = 0xFF0A0E1A, C_ACCENTO = 0xFF5AA9FF, C_TENUE = 0xFF6B7797,
            C_SCHEDA = 0xFF0E1424, C_BORDO = 0xFF1B2740, C_SU_ACCENTO = 0xFFFFFFFF;

    private WebView web;
    private TextView stato;
    private String segreto;
    private boolean caricata = false, pronto = false, entraAppenaPronto = true, inPausa = false;
    private volatile boolean avviando = false, provatoInPrimoPiano = false;

    private ValueCallback<Uri[]> attesaFile;
    private Uri scattoInCorso;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
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
             + "|| bash ~/nyctalus/nyctalus-mobile --sfondo)";
    }

    // ------------------------------------------------------------- AVVIO -----
    private View costruisciAvvio() {
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.VERTICAL);
        r.setGravity(Gravity.CENTER_HORIZONTAL);
        r.setBackgroundColor(C_SFONDO);
        r.setPadding(dp(22), dp(22), dp(22), dp(20));
        r.addView(spazio());

        ImageView logo = new ImageView(this);
        logo.setImageResource(R.drawable.logo);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(dp(112), dp(112));
        lp.bottomMargin = dp(14);
        logo.setLayoutParams(lp);
        r.addView(logo);

        TextView t = testo("Nyctalus", C_ACCENTO, 30);
        t.setTypeface(t.getTypeface(), Typeface.BOLD);
        r.addView(t);
        TextView sub = testo("chat anonima", C_TENUE, 14);
        sub.setLetterSpacing(0.2f);
        r.addView(sub);

        stato = testo("avvio in corso…", C_TENUE, 13);
        LinearLayout.LayoutParams sp = avvolgi();
        sp.topMargin = dp(22);
        stato.setLayoutParams(sp);
        r.addView(stato);

        Button entra = bottone("Entra  ▸", true, v -> entra());
        LinearLayout.LayoutParams ep = avvolgi();
        ep.topMargin = dp(22);
        entra.setLayoutParams(ep);
        r.addView(entra);
        r.addView(spazio());

        LinearLayout riga1 = riga(), riga2 = riga();
        riga1.addView(cella("⌨  Apri Termux", v -> apriTermux()));
        riga1.addView(cella("⬇  Installa Termux", v -> installaTermux()));
        riga2.addView(cella("📋  Copia il comando di avvio", v -> copiaComando()));
        ((LinearLayout.LayoutParams) riga2.getLayoutParams()).topMargin = dp(10);
        r.addView(riga1);
        r.addView(riga2);
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
                  + "Se non l'hai ancora installato in Termux, segui il manuale.\n"
                  + "Altrimenti: «Copia il comando di avvio», incollalo in Termux e torna qui.");
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
            return c.getResponseCode() == 200;
        } catch (IOException e) {
            return false;
        } finally {
            if (c != null) c.disconnect();
        }
    }

    private void servizioPronto() {
        pronto = true;
        imposta("Nyctalus è pronto ✓");
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

    private void installaTermux() {
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse("https://f-droid.org/packages/com.termux/")));
        } catch (Exception e) {
            avviso("Apri: f-droid.org/packages/com.termux");
        }
    }

    /** Ripiego sempre valido: il comando negli appunti, da incollare in Termux. */
    private void copiaComando() {
        copiaNegliAppunti(comandoAvvio());
        apriTermux();
        avviso("Comando copiato. In Termux: tieni premuto → Incolla → Invio, poi torna qui.");
    }

    // ----------------------------------------------------------- WEB-APP -----
    private void mostraWeb() {
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
        if (tasto == KeyEvent.KEYCODE_BACK && web != null && web.canGoBack()) {
            web.goBack();
            return true;
        }
        return super.onKeyDown(tasto, e);
    }
}
