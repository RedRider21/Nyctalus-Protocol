// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
// Derivato dal browser di NovaOS (Copyright (c) 2026 RedRider21, licenza MIT), con in più
// la modalità Nyctalus: tutto il traffico passa dalla rete Nyctalus (vedi «Nyctalus» qui sotto).
package org.nyctalus.mobile;

import android.app.Activity;
import android.app.AlertDialog;
import android.app.DownloadManager;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.content.res.ColorStateList;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.PorterDuff;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.net.Uri;
import android.os.Bundle;
import android.os.Environment;
import android.os.Message;
import android.text.Editable;
import android.text.InputType;
import android.text.TextUtils;
import android.text.TextWatcher;
import android.util.Log;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewGroup.LayoutParams;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.webkit.CookieManager;
import android.webkit.GeolocationPermissions;
import android.webkit.PermissionRequest;
import android.webkit.URLUtil;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebStorage;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.CheckBox;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.GridLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.PopupWindow;
import android.widget.ProgressBar;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONArray;
import org.json.JSONObject;
import org.json.JSONTokener;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.text.SimpleDateFormat;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.net.InetSocketAddress;
import java.net.Proxy;
import java.security.SecureRandom;

/**
 * Il browser di NovaOS.
 *
 * <p>È lo stesso browser della traccia Gecko ({@code :gecko}), sull'altro motore: la
 * schermata, le icone, il menu, i preferiti condivisi con la shell, la scelta dei file e i
 * permessi di camera, microfono e posizione sono gli stessi. Quello che cambia è cosa c'è
 * sotto — e cambia in meglio, perché la WebView è un motore e una scatola insieme: sa dire
 * da sé cosa sta mostrando ({@code getUrl()}, {@code getTitle()}, {@code canGoBack()}) e
 * disegnare la propria anteprima, quindi metà del lavoro che Gecko ha richiesto qui non
 * esiste. Le differenze che restano sono tre, e sono tutte a sfavore della WebView:
 *
 * <ul>
 *   <li><b>Una vista per scheda.</b> La WebView non si può spostare fra due contenitori
 *       restando la stessa: qui ogni scheda ha la sua, e cambiare scheda vuol dire staccare
 *       una vista e agganciarne un'altra. Dieci schede costano dieci superfici di disegno.</li>
 *   <li><b>I popup passano da {@code onCreateWindow}</b>, che consegna un messaggio da
 *       rimandare indietro con la WebView che dovrà disegnare la pagina nuova. Se non lo si
 *       fa, {@code window.open} resta appeso senza errore.</li>
 *   <li><b>I download</b> passano dal {@code DownloadListener}, che consegna solo un
 *       indirizzo: il file lo scarica il sistema, senza i cookie della sessione (un PDF
 *       dietro login non si scarica). Sotto Gecko arriva lo stream già aperto. È un limite
 *       dichiarato, non una scelta.</li>
 * </ul>
 *
 * <p>Nyctalus: aperto dall'app ({@code MainActivity}) in un PROCESSO SEPARATO (":browser"),
 * perché il proxy della WebView vale per tutto il processo. In modalità Nyctalus ogni
 * scheda è in incognito, il traffico passa dall'inoltratore SOCKS dell'orchestratore
 * (verificato prima di aprire qualunque pagina: se non passa da lì, non si apre nulla),
 * WebRTC è spento prima che parta la pagina, niente permessi a fotocamera/microfono/
 * posizione, e anche traduzione e download passano dalla rete.
 */
public class BrowserActivity extends Activity {

    // ================================================================== Nyctalus
    /** Modalità Nyctalus: TUTTO il traffico di questo processo passa dalla rete. */
    private static boolean nyctalus = false;
    private int portaSocks = 18733;
    private TextView nyctBtn;
    private static final String SEARCH_NYCT = "https://duckduckgo.com/?q=";
    private static final String HOME_NYCT = "https://duckduckgo.com/";
    /** Pagina che esiste solo dentro l'inoltratore dell'orchestratore (vedi socks_browser). */
    private static final String VERIFICA = "http://verifica.nyctalus.invalid/";
    /** WebRTC parla in UDP fuori dal proxy e rivelerebbe l'indirizzo vero: si toglie. */
    private static final String JS_NO_WEBRTC = "(function(){try{"
        + "['RTCPeerConnection','webkitRTCPeerConnection','RTCDataChannel','RTCSessionDescription','RTCIceCandidate']"
        + ".forEach(function(k){try{Object.defineProperty(window,k,{value:undefined,writable:false,configurable:false});}catch(e){}});"
        + "try{Object.defineProperty(navigator,'mediaDevices',{value:undefined,writable:false,configurable:false});}catch(e){}"
        + "try{Object.defineProperty(navigator,'geolocation',{value:undefined,writable:false,configurable:false});}catch(e){}"
        + "}catch(e){}})();";

    private Proxy proxyJava() {
        return nyctalus ? new Proxy(Proxy.Type.SOCKS, InetSocketAddress.createUnresolved("127.0.0.1", portaSocks)) : Proxy.NO_PROXY;
    }

    /** Le connessioni fatte dal codice (traduzione, manifest, download): in modalità
     *  Nyctalus anche loro dentro la rete, con il nome risolto dall'altra parte. */
    private java.net.URLConnection apri(URL u) throws java.io.IOException {
        if (!nyctalus) return u.openConnection();
        return u.openConnection(new Proxy(Proxy.Type.SOCKS, new InetSocketAddress("127.0.0.1", portaSocks)));
    }

    private static boolean eNyct(String url) {
        String h = Uri.parse(url == null ? "" : url).getHost();
        return h != null && h.toLowerCase(Locale.ROOT).endsWith(".nyct");
    }

    private void aggiornaPulsanteNyctalus() {
        if (nyctBtn == null) return;
        GradientDrawable g = new GradientDrawable();
        g.setCornerRadius(dp(14));
        g.setColor(nyctalus ? 0xFF1f8a4c : 0x00000000);
        g.setStroke(dp(1), nyctalus ? 0xFF3ec46d : DIM);
        nyctBtn.setBackground(g);
        nyctBtn.setTextColor(nyctalus ? 0xFFFFFFFF : DIM);
        nyctBtn.setText(nyctalus ? "🦇 ON" : "🦇");
    }

    private void chiudiTutteLeSchede() {
        holder.removeAllViews();
        for (Scheda t : schede) { try { t.web.destroy(); } catch (Exception e) { /* già distrutta */ } }
        schede.clear();
        corrente = -1;
    }

    /** Cookie, archivi e cache delle pagine: via (all'uscita dalla modalità Nyctalus). */
    private void cancellaDatiNavigazione() {
        try { CookieManager.getInstance().removeAllCookies(null); CookieManager.getInstance().flush(); } catch (Exception e) { }
        try { android.webkit.WebStorage.getInstance().deleteAllData(); } catch (Exception e) { }
        try { WebView w = new WebView(this); w.clearCache(true); w.clearFormData(); w.destroy(); } catch (Exception e) { }
    }

    private void cambiaNyctalus() {
        if (nyctalus) {
            new AlertDialog.Builder(this, AlertDialog.THEME_DEVICE_DEFAULT_DARK)
                .setTitle("Uscire dalla modalità Nyctalus?")
                .setMessage("Le schede aperte si chiudono e i loro dati (cookie, cache) si cancellano. Poi navighi normalmente, senza la rete Nyctalus.")
                .setPositiveButton("Esci", (d, w) -> disattivaNyctalus())
                .setNegativeButton("Resta", null).show();
        } else {
            new AlertDialog.Builder(this, AlertDialog.THEME_DEVICE_DEFAULT_DARK)
                .setTitle("🦇 Modalità Nyctalus")
                .setMessage("Tutto il traffico del browser passerà dalla rete Nyctalus: il sito non vede il tuo indirizzo. "
                    + "Le schede normali si chiudono; in questa modalità niente cronologia, niente WebRTC, niente fotocamera o posizione, "
                    + "e i siti .nyct si aprono.\n\nServe Nyctalus acceso (nell'app: Entra, o la rete pronta).")
                .setPositiveButton("Attiva", (d, w) -> attivaNyctalus(null))
                .setNegativeButton("Annulla", null).show();
        }
    }

    /** Imposta il proxy, poi VERIFICA che passi davvero da Nyctalus; solo dopo apre pagine. */
    private void attivaNyctalus(final String url) {
        if (!ProxyNyctalus.supporta("PROXY_OVERRIDE")) {
            fallitaNyctalus("La WebView di questo telefono non permette di instradare il traffico "
                + "(serve Android 9 o successivo e «Android System WebView»/Chrome aggiornati dal Play Store).");
            return;
        }
        Toast.makeText(this, "Collego il browser alla rete Nyctalus…", Toast.LENGTH_SHORT).show();
        chiudiTutteLeSchede();
        try {
            ProxyNyctalus.imposta("127.0.0.1", portaSocks, () -> verificaNyctalus(url), getMainExecutor());
        } catch (Throwable e) {
            Log.w(TAG, "proxy non impostato", e);
            fallitaNyctalus("Impostazione del proxy non riuscita: " + e);
        }
    }

    private void verificaNyctalus(final String url) {
        byte[] b = new byte[8];
        new SecureRandom().nextBytes(b);
        StringBuilder sb = new StringBuilder();
        for (byte x : b) sb.append(String.format("%02x", x));
        final String gettone = sb.toString();
        final WebView prova = new WebView(this);
        prova.getSettings().setJavaScriptEnabled(true);
        prova.getSettings().setCacheMode(WebSettings.LOAD_NO_CACHE);
        final AtomicBoolean finita = new AtomicBoolean(false);
        final android.os.Handler h = new android.os.Handler(getMainLooper());
        final java.util.function.Consumer<Boolean> chiudi = ok -> {
            if (!finita.compareAndSet(false, true)) return;
            try { prova.destroy(); } catch (Exception e) { }
            if (ok) entraInNyctalus(url);
            else fallitaNyctalus("Il traffico non passa dalla rete Nyctalus (Nyctalus è acceso? Nell'app tocca Entra e "
                + "attendi «rete pronta»). Per sicurezza non è stata aperta nessuna pagina.");
        };
        prova.setWebViewClient(new WebViewClient() {
            @Override public void onPageFinished(WebView vw, String u) {
                vw.evaluateJavascript("document.body?document.body.innerText:''",
                    r -> chiudi.accept(r != null && r.contains("nyctalus-ok /" + gettone)));
            }
            @Override public void onReceivedError(WebView vw, WebResourceRequest req, WebResourceError err) {
                if (req != null && req.isForMainFrame()) chiudi.accept(false);
            }
        });
        h.postDelayed(() -> chiudi.accept(false), 20000);
        prova.loadUrl(VERIFICA + gettone);
    }

    private void entraInNyctalus(String url) {
        nyctalus = true;
        cancellaDatiNavigazione();
        aggiornaPulsanteNyctalus();
        nuovaScheda(url != null && !url.isEmpty() ? url : HOME_NYCT, true, false);
        Toast.makeText(this, "🦇 Modalità Nyctalus attiva: il traffico passa dalla rete", Toast.LENGTH_LONG).show();
    }

    private void fallitaNyctalus(String perche) {
        nyctalus = false;
        try { ProxyNyctalus.togli(() -> { }, getMainExecutor()); } catch (Throwable e) { /* non era impostato */ }
        aggiornaPulsanteNyctalus();
        new AlertDialog.Builder(this, AlertDialog.THEME_DEVICE_DEFAULT_DARK)
            .setTitle("Modalità Nyctalus non attivata")
            .setMessage(perche)
            .setPositiveButton("OK", (d, w) -> { if (schede.isEmpty()) nuovaScheda(paginaIniziale(), false, false); })
            .setCancelable(false).show();
    }

    private void disattivaNyctalus() {
        chiudiTutteLeSchede();
        cancellaDatiNavigazione();
        try {
            ProxyNyctalus.togli(() -> {
                nyctalus = false;
                aggiornaPulsanteNyctalus();
                nuovaScheda(paginaIniziale(), false, false);
            }, getMainExecutor());
        } catch (Throwable e) {
            // il proxy non si toglie: per sicurezza si chiude il browser (il processo riparte pulito)
            finishAndRemoveTask();
        }
    }

    /** Il browser stava in un processo suo: queste due chiamate erano per la shell di NovaOS. */
    private void preferitiCambiati() { }
    private void appInstallate() { }


    private static final String TAG = "NyctalusBrowser";

    // ---- una scheda del browser ----
    private static class Scheda {
        WebView web;
        String titolo = "Nuova scheda";
        String indirizzo = "";
        boolean incognito = false;
        boolean desktop = false;
        Bitmap anteprima;   // per il selettore schede
        // Scheda di lettura: dentro non c'è un sito da visitare ma un testo che abbiamo
        // ricavato noi. Serve a due cose: non finire nella cronologia (l'indirizzo è quello
        // del sito, ma la pagina visitata è questa) e non riaprire una lettura su una lettura.
        boolean lettura = false;
        // Traduzione. «linguaPagina» è quella che la pagina dichiara di essere (attributo
        // lang): serve a decidere se proporre la traduzione, e si legge dalla pagina, senza
        // mandare niente a nessuno. «linguaMostrata» è la lingua in cui la pagina è a video
        // adesso: vuota vuol dire «originale». Stanno qui e non in una variabile della
        // schermata perché ogni scheda ha la sua pagina, e cambiando scheda non si deve
        // ereditare la traduzione dell'altra.
        String linguaPagina = "";
        String linguaMostrata = "";
        // L'ultima traduzione di questa pagina, tenuta qui: rimettere l'originale e poi
        // ritradurre nella stessa lingua non deve costare altre richieste al servizio.
        List<String> blocchiTradotti = null;
        String linguaBlocchi = "";
    }

    private FrameLayout selettore;   // selettore schede a griglia (null = chiuso)
    private PopupWindow menuAperto;  // il pannello di ⋮, quando è aperto

    private final List<Scheda> schede = new ArrayList<>();
    private int corrente = -1;

    private FrameLayout holder;      // contiene la WebView della scheda attiva
    private EditText omnibox;        // barra indirizzo/ricerca editabile
    private ImageView secIco;        // lucchetto / «i» sicurezza, accanto all'indirizzo
    private Button tabBtn;           // contatore schede -> selettore
    private LinearLayout bar;        // barra superiore (tema chiaro/incognito)
    private LinearLayout cap;        // capsula omnibox
    private ImageView incBadge;      // occhiali: la scheda in vista è in incognito
    private ProgressBar progress;    // avanzamento caricamento
    private LinearLayout findBar;    // barra "trova nella pagina"
    private EditText findInput;
    private TextView findInfo;       // "3/12": quante occorrenze e quale si sta guardando
    // barra della traduzione: compare sotto quella superiore quando la pagina parla
    // un'altra lingua. Il testo dice cosa sta succedendo, il pulsante cosa si può fare.
    private LinearLayout tradBar;
    private TextView tradTesto;
    private Button tradAzione;      // «Traduci» / «Mostra originale» / spento durante il lavoro
    private ImageView tradIcona;

    /** Perché la traduzione non è riuscita, da dire nella barra. Si azzera a ogni tentativo:
     *  un messaggio vecchio su un guasto nuovo sarebbe una mezza verità. */
    private String tradMessaggio = "";

    private String mobileUa;

    private static final int ACC = 0xFF0a84ff;
    private static final String SEARCH = "https://duckduckgo.com/?q=";

    /** Dove porta la casa finché non si sceglie una pagina iniziale. */
    private static final String HOME_PREDEFINITA = "https://duckduckgo.com/";

    /** Il browser desktop. Non è la stringa di un altro browser: è quella con cui questi
     *  siti si comportavano bene qui, e cambiarla cambierebbe l'impaginazione dei siti che
     *  leggono il nome del browser. */
    private static final String UA_DESKTOP =
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

    // Palette dinamica: segue il tema di NovaOS (chiaro/scuro), impostata da initTheme().
    // BG=sfondo, BAR=barra, TXT=testo, DIM=testo attenuato, CAP=capsula omnibox,
    // CARD=scheda nel selettore, PH=placeholder anteprima.
    private int BG, BAR, TXT, DIM, CAP, CARD, PH;
    // tema incognito (barra scura viola, capsula più scura) — sempre scuro
    private static final int INC_BG = 0xFF17141f, INC_BAR = 0xFF2a2438, INC_CAP = 0xFF3a3350;
    // Testo e icone dell'incognito: servono per forza, non si ricavano dal tema. La barra
    // dell'incognito è scura sempre, mentre la palette qui sopra segue NovaOS — con NovaOS
    // in chiaro il testo della barra era nero su viola scuro, cioè quasi invisibile.
    private static final int INC_TXT = 0xFFeceaf4, INC_DIM = 0xFFb3aec6, INC_CARD = 0xFF231d33;

    // Viste che devono cambiare colore quando cambia la palette in uso (il tema di NovaOS,
    // oppure quella dell'incognito): si registrano mentre si costruiscono — sono poche e
    // note — invece di andarle a cercare a ogni cambio.
    private final List<TextView> testiTema = new ArrayList<>();
    private final List<ImageView> iconeTema = new ArrayList<>();
    private Button reloadBtn, piuBtn, menuBtn;
    // La stella dei preferiti del menu. Il pannello resta aperto quando la si tocca, quindi
    // la stella va aggiornata sul posto: una stella che non cambia si legge come «non è
    // successo niente», ed è il difetto che questa coppia di riferimenti chiude.
    private TextView stellaFila, stellaVoceIcona, stellaVoceTesto;
    // Palette del pannello del menu mentre lo si costruisce (segue la scheda in vista).
    private int pCard, pCap, pTxt, pDim;

    /**
     * Le decisioni prese sito per sito (v. «Impostazioni del sito»).
     *
     * <p>Sta nella stessa copia dei preferiti e del tema: un oggetto con una chiave per
     * sito — {@code {"example.com":{"popup":"block","camera":"block","desktop":true}}} — e
     * dentro solo le voci che l'utente ha davvero toccato. Un sito senza chiave è un sito
     * su cui non si è deciso niente, e vale il comportamento di sempre: è il motivo per cui
     * le decisioni si salvano per differenza e non per valore di partenza.
     */
    private static final String PREF_SITI = "nova:browserSiti";

    /** La lingua in cui si traduce: è una scelta dell'utente, e resta finché non la cambia. */
    private static final String PREF_LINGUA = "nova:browserLingua";

    /** I siti per cui non si deve tradurre: un elenco di indirizzi. */
    private static final String PREF_NO_TRAD = "nova:browserNoTraduzione";

    /**
     * Le lingue che si possono scegliere.
     *
     * <p>Non è l'elenco completo del servizio, e non vuole esserlo: sono le lingue in cui
     * una persona ha davvero motivo di leggere, e ognuna è già scritta nella lingua sua
     * accanto al nome italiano — chi cerca il polacco cerca «polski» o «polski», non
     * necessariamente «polacco». Il codice è quello che si manda al servizio.
     */
    private static final String[][] LINGUE = {
        { "it", "Italiano" },        { "en", "Inglese" },
        { "es", "Spagnolo" },        { "fr", "Francese" },
        { "de", "Tedesco" },         { "pt", "Portoghese" },
        { "nl", "Olandese" },        { "sv", "Svedese" },
        { "da", "Danese" },          { "nb", "Norvegese" },
        { "fi", "Finlandese" },      { "is", "Islandese" },
        { "ga", "Irlandese" },       { "cy", "Gallese" },
        { "el", "Greco" },           { "pl", "Polacco" },
        { "cs", "Ceco" },            { "sk", "Slovacco" },
        { "hu", "Ungherese" },       { "ro", "Rumeno" },
        { "bg", "Bulgaro" },         { "hr", "Croato" },
        { "sr", "Serbo" },           { "sl", "Sloveno" },
        { "bs", "Bosniaco" },        { "mk", "Macedone" },
        { "sq", "Albanese" },        { "ru", "Russo" },
        { "uk", "Ucraino" },         { "be", "Bielorusso" },
        { "lt", "Lituano" },         { "lv", "Lettone" },
        { "et", "Estone" },          { "tr", "Turco" },
        { "az", "Azero" },           { "kk", "Kazako" },
        { "hy", "Armeno" },          { "ka", "Georgiano" },
        { "he", "Ebraico" },         { "ar", "Arabo" },
        { "fa", "Persiano" },        { "ur", "Urdu" },
        { "hi", "Hindi" },           { "bn", "Bengalese" },
        { "ta", "Tamil" },           { "te", "Telugu" },
        { "mr", "Marathi" },         { "gu", "Gujarati" },
        { "pa", "Punjabi" },         { "ne", "Nepalese" },
        { "si", "Singalese" },       { "th", "Thailandese" },
        { "vi", "Vietnamita" },      { "id", "Indonesiano" },
        { "ms", "Malese" },          { "tl", "Filippino" },
        { "zh-CN", "Cinese (semplificato)" }, { "zh-TW", "Cinese (tradizionale)" },
        { "ja", "Giapponese" },      { "ko", "Coreano" },
        { "sw", "Swahili" },         { "am", "Amarico" },
        { "af", "Afrikaans" },       { "eo", "Esperanto" }
    };

    /**
     * Nomi delle lingue che la pagina può dichiarare, per parlarne all'utente.
     *
     * <p>L'attributo {@code lang} di una pagina è una sigla: «en», «en-GB», «pt-BR». Qui si
     * traduce la sigla in un nome, e si guarda solo la parte prima del trattino: dire
     * «inglese britannico» quando la pagina dice solo «inglese» sarebbe aggiungere
     * un'informazione che non c'è.
     */
    private static String nomeLingua(String codice) {
        if (codice == null) return "";
        String c = codice.trim();
        if (c.isEmpty()) return "";
        if (c.contains("-") || c.contains("_")) c = c.split("[-_]")[0];
        for (String[] l : LINGUE) {
            if (l[0].equalsIgnoreCase(c)) return l[1].toLowerCase(Locale.ITALY);
        }
        return "";
    }

    /** Legge il tema salvato da NovaOS (SharedPreferences "novaos", chiave nova:theme)
     *  e sceglie la palette chiara o scura, per andare di pari passo con il resto del sistema. */
    private void initTheme() {
        String t = getSharedPreferences("nyctalus", MODE_PRIVATE).getString("tema", "notte");
        boolean light = "chiaro".equals(t);
        if (light) {
            BG = 0xFFf2f4f8; BAR = 0xFFffffff; TXT = 0xFF141a24; DIM = 0xFF5b6472;
            CAP = 0xFFe9edf3; CARD = 0xFFffffff; PH = 0xFFe4e8ef;
        } else {
            BG = 0xFF0b0f17; BAR = 0xFF151a24; TXT = 0xFFe8ecf4; DIM = 0xFF9aa4b8;
            CAP = 0xFF232937; CARD = 0xFF1c2331; PH = 0xFF0f141d;
        }
        // Il menu non è ancora costruito: la sua palette parte da quella del tema e viene
        // sostituita all'apertura, se la scheda in vista è in incognito.
        pCard = CARD; pCap = CAP; pTxt = TXT; pDim = DIM;
    }

    private static boolean cartellaWebImpostata = false;

    @Override protected void onCreate(Bundle b) {
        // Processo separato (":browser"): da Android 9 due processi della stessa app NON
        // possono usare la WebView con la stessa cartella dati (la chat la usa nel processo
        // principale) — senza questa riga Android chiude il browser appena parte.
        if (!cartellaWebImpostata && android.os.Build.VERSION.SDK_INT >= 28) {
            try { WebView.setDataDirectorySuffix("browser"); } catch (Exception e) { Log.w(TAG, "cartella WebView", e); }
            cartellaWebImpostata = true;
        }
        // «Scatola nera»: se il browser si chiude per un errore, l'app lo mostra alla
        // riapertura (MainActivity.mostraErroreBrowser), così si può riferire.
        final Thread.UncaughtExceptionHandler prima = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler((th, ex) -> {
            try (java.io.PrintWriter pw = new java.io.PrintWriter(new java.io.File(getFilesDir(), "errore-browser.txt"))) {
                pw.println("Android " + android.os.Build.VERSION.RELEASE + " (API " + android.os.Build.VERSION.SDK_INT + ")");
                ex.printStackTrace(pw);
            } catch (Exception ignorata) { }
            if (prima != null) prima.uncaughtException(th, ex);
        });
        super.onCreate(b);
        initTheme();   // palette in base al tema di NovaOS, prima di costruire la UI

        LinearLayout rootv = new LinearLayout(this);
        rootv.setOrientation(LinearLayout.VERTICAL);
        rootv.setBackgroundColor(BG);

        // ---- barra superiore: [casa] [ omnibox ] [＋] [contatore] [⋮] ----
        bar = new LinearLayout(this);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        bar.setGravity(Gravity.CENTER_VERTICAL);
        bar.setBackgroundColor(BAR);
        bar.setPadding(dp(8), dp(10), dp(8), dp(10));

        // Occhiali «in incognito»: visibili solo quando la scheda in vista è in incognito.
        // Accanto al contatore delle schede, non in mezzo alle altre icone, perché dicono
        // della scheda che il contatore conta.
        incBadge = iconaVista(R.drawable.ic_incognito, 20, TXT);
        incBadge.setPadding(dp(2), 0, dp(2), 0);
        incBadge.setVisibility(View.GONE);

        tabBtn = new Button(this);
        tabBtn.setText("1");
        tabBtn.setAllCaps(false);
        tabBtn.setBackground(squareBadge());
        tabBtn.setTextColor(TXT);
        tabBtn.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        tabBtn.setMinWidth(dp(34)); tabBtn.setMinimumWidth(dp(34));
        tabBtn.setPadding(dp(4), 0, dp(4), 0);
        tabBtn.setOnClickListener(v -> mostraSelettore());

        // capsula omnibox
        cap = new LinearLayout(this);
        cap.setOrientation(LinearLayout.HORIZONTAL);
        cap.setGravity(Gravity.CENTER_VERTICAL);
        GradientDrawable capBg = new GradientDrawable();
        capBg.setColor(CAP); capBg.setCornerRadius(dp(22));
        cap.setBackground(capBg);
        cap.setPadding(dp(14), 0, dp(6), 0);
        LinearLayout.LayoutParams capLp = new LinearLayout.LayoutParams(0, dp(42), 1f);
        capLp.leftMargin = dp(6); capLp.rightMargin = dp(4);
        cap.setLayoutParams(capLp);

        secIco = iconaVista(R.drawable.ic_lock, 15, DIM);   // colore suo: lo decide syncBar

        omnibox = new EditText(this);
        omnibox.setSingleLine(true);
        omnibox.setBackgroundColor(Color.TRANSPARENT);
        omnibox.setTextColor(TXT);
        omnibox.setHint("Cerca o digita un indirizzo");
        omnibox.setHintTextColor(DIM);
        omnibox.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        omnibox.setInputType(InputType.TYPE_TEXT_VARIATION_URI | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        // Il fuoco lo prende il dito, non l'avvio della schermata: senza questa riga è il
        // primo campo dell'Activity a riceverlo, e da quel momento «il campo ha il fuoco»
        // smette di voler dire «l'utente sta scrivendo» — cioè syncBar non aggiorna più
        // l'indirizzo, e dopo un cambio scheda resta lì quello vecchio, selezionato.
        omnibox.setFocusableInTouchMode(true);
        omnibox.setImeOptions(EditorInfo.IME_ACTION_GO);
        omnibox.setPadding(dp(10), 0, dp(6), 0);
        omnibox.setLayoutParams(new LinearLayout.LayoutParams(0, LayoutParams.MATCH_PARENT, 1f));
        omnibox.setOnEditorActionListener((tv, id, ev) -> {
            if (id == EditorInfo.IME_ACTION_GO || id == EditorInfo.IME_NULL) { naviga(omnibox.getText().toString()); return true; }
            return false;
        });
        omnibox.setOnFocusChangeListener((v, has) -> { if (has) omnibox.selectAll(); });

        reloadBtn = iconBtn("⟳");
        reloadBtn.setOnClickListener(v -> { Scheda s = schedaCorrente(); if (s != null && s.web != null) s.web.reload(); });

        cap.addView(secIco);
        cap.addView(omnibox);
        cap.addView(reloadBtn);
        testiTema.add(reloadBtn);

        // casa: riporta alla pagina iniziale. È il primo elemento della barra: la si
        // cerca a sinistra dell'omnibox, non in mezzo alle altre icone.
        ImageView casa = iconaBtn(R.drawable.ic_home);
        casa.setOnClickListener(v -> vaiAllaHome());
        iconeTema.add(casa);
        // Gli occhiali dell'incognito stanno nella stessa fila: la visibilità la decide
        // applicaTemaBarra, il colore lo ricevono come le altre icone.
        iconeTema.add(incBadge);

        // ＋ apre una scheda nuova. La stella dei preferiti non sta più qui: era l'unica
        // icona che cambiava aspetto da sola a ogni pagina, e il preferito si aggiunge
        // dalle due voci del menu (che si aprono da ⋮).
        piuBtn = iconBtn("＋");
        piuBtn.setOnClickListener(v -> nuovaScheda(paginaIniziale(), false, false));

        menuBtn = iconBtn("⋮");
        menuBtn.setOnClickListener(this::mostraMenu);

        // L'ordine della fila: la casa a sinistra (è lì che la si cerca),
        // l'omnibox a prendersi lo spazio che avanza, ＋ subito dopo, e in fondo il
        // contatore delle schede attaccato a ⋮ — con gli occhiali dell'incognito accanto,
        // perché dicono della scheda che il contatore conta.
        // 🦇 modalità Nyctalus: verde quando il traffico passa dalla rete
        nyctBtn = new TextView(this);
        nyctBtn.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        nyctBtn.setGravity(Gravity.CENTER);
        nyctBtn.setPadding(dp(8), dp(4), dp(8), dp(4));
        nyctBtn.setOnClickListener(v -> cambiaNyctalus());
        aggiornaPulsanteNyctalus();
        bar.addView(nyctBtn);
        bar.addView(casa);
        bar.addView(cap);
        bar.addView(piuBtn);
        bar.addView(incBadge);
        bar.addView(tabBtn);
        bar.addView(menuBtn);
        testiTema.add(piuBtn);
        testiTema.add(menuBtn);

        // barra di avanzamento
        progress = new ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal);
        progress.setLayoutParams(new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, dp(3)));
        progress.setMax(100);
        progress.setProgressTintList(ColorStateList.valueOf(ACC));
        progress.setProgressBackgroundTintList(ColorStateList.valueOf(BAR));
        progress.setVisibility(View.GONE);

        holder = new FrameLayout(this);
        holder.setLayoutParams(new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f));

        // barra "trova nella pagina" (nascosta)
        findBar = buildFindBar();
        // barra della traduzione (nascosta): sta sotto quella di ricerca, come in un
        // browser vero, e non la sostituisce — si può cercare dentro una pagina tradotta.
        tradBar = creaTradBar();

        rootv.addView(bar);
        rootv.addView(progress);
        rootv.addView(findBar);
        rootv.addView(tradBar);
        rootv.addView(holder);
        setContentView(rootv);

        // La schermata nasce con il fuoco in nessun campo, e non sull'omnibox.
        //
        // Il fuoco è la sola spia che abbiamo di «l'utente sta scrivendo»: syncBar aggiorna
        // l'indirizzo solo se il campo non ha il fuoco, perché altrimenti le notifiche di
        // caricamento cancellerebbero quello che si sta digitando. Ma in touch mode il primo
        // campo dell'Activity è anche il primo a prendere il fuoco da solo: la schermata si
        // apriva con l'omnibox vuota e focalizzata, e l'indirizzo non ci finiva mai — una
        // barra che sembra rotta mentre la pagina è già lì. Il fuoco lo prende il
        // contenitore, che non ha nulla da mostrare.
        rootv.setFocusableInTouchMode(true);
        rootv.requestFocus();

        String url = getIntent().getStringExtra("url");
        if (url == null || url.isEmpty()) url = HOME_PREDEFINITA;
        if (!url.matches("^[a-zA-Z]+://.*")) url = "https://" + url;
        boolean desk = getIntent().getBooleanExtra("desktop", false) || vuoleDesktop(url);
        portaSocks = getIntent().getIntExtra("porta_socks", portaSocks);
        if (getIntent().getBooleanExtra("nyctalus", false) || eNyct(url)) {
            attivaNyctalus(getIntent().hasExtra("url") ? url : null);
        } else {
            nuovaScheda(url, false, desk);
        }
    }

    // ------------------------------------------------------------------ schede
    private Scheda schedaCorrente() { return corrente >= 0 && corrente < schede.size() ? schede.get(corrente) : null; }

    private Scheda nuovaScheda(String url, boolean incognito, boolean desktop) {
        if (nyctalus) incognito = true;
        Scheda t = new Scheda();
        t.incognito = incognito;
        t.desktop = desktop;
        t.web = buildWebView(t);
        schede.add(t);
        mostraScheda(schede.size() - 1);
        if (url != null && !url.isEmpty()) { t.indirizzo = url; t.web.loadUrl(url); }
        return t;
    }

    /**
     * Aggancia la scheda indicata: la sua WebView va nel contenitore, quella che si lascia
     * ne esce.
     *
     * <p>L'anteprima della scheda che si sta lasciando si cattura <b>prima</b> dello scambio:
     * è ancora a schermo, quindi il disegno è quello giusto. È la stessa ragione per cui
     * sotto Gecko la si cattura prima di staccare la sessione.
     */
    private void mostraScheda(int i) {
        if (i < 0 || i >= schede.size()) return;
        if (corrente >= 0 && corrente < schede.size() && corrente != i) catturaAnteprima(schede.get(corrente));
        corrente = i;
        Scheda t = schede.get(i);
        holder.removeAllViews();
        if (t.web.getParent() != null) ((ViewGroup) t.web.getParent()).removeView(t.web);
        holder.addView(t.web, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        tabBtn.setText(String.valueOf(schede.size()));
        LinearLayout rv = radice();
        if (rv != null) rv.setBackgroundColor(t.incognito ? INC_BG : BG);
        // Cambio scheda: l'indirizzo della scheda che si apre deve comparire subito, anche se
        // il campo aveva il fuoco. Chi sta scrivendo lo perde, ed è giusto: ha appena scelto
        // un'altra scheda.
        omnibox.clearFocus();
        syncBar(t.indirizzo);
        // La barra della traduzione è della scheda, non della schermata: cambiando scheda si
        // rimette quella della scheda che entra (o sparisce, se non c'è niente da dire).
        if (!t.linguaMostrata.isEmpty()) barraTraduzione(t, T_TRADOTTA);
        else mostraBarraTraduzione(false);
    }

    private void chiudiScheda(int i) {
        if (i < 0 || i >= schede.size()) return;
        Scheda t = schede.remove(i);
        holder.removeAllViews();
        try { t.web.destroy(); } catch (Exception e) { /* già distrutta */ }
        if (schede.isEmpty()) { finish(); return; }
        corrente = -1;
        mostraScheda(Math.max(0, i - 1));
    }

    // ---- selettore schede: griglia di anteprime con X per chiudere ----
    private void mostraSelettore() {
        if (selettore != null) return;
        catturaAnteprima(schedaCorrente());   // anteprima aggiornata della scheda corrente

        FrameLayout overlay = new FrameLayout(this);
        // Opaco, non un velo: sotto c'è la barra del browser, e con un velo al 95% la si
        // vede trasparire dietro il titolo — due barre sovrapposte, con dentro due volte
        // lo stesso conto delle schede.
        overlay.setBackgroundColor(BG);
        overlay.setClickable(true);

        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);

        // barra superiore: titolo + nuova scheda + nuova incognito + chiudi
        LinearLayout top = new LinearLayout(this);
        top.setOrientation(LinearLayout.HORIZONTAL);
        top.setGravity(Gravity.CENTER_VERTICAL);
        top.setPadding(dp(14), dp(16), dp(14), dp(8));
        TextView title = new TextView(this);
        // Senza il numero: il conto lo dà già l'intestazione di ciascun gruppo qui sotto,
        // e ripeterlo due volte nella stessa schermata sembra un errore.
        title.setText("Schede");
        title.setTextColor(TXT); title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        title.setLayoutParams(new LinearLayout.LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f));
        Button addBtn = iconBtn("＋"); addBtn.setOnClickListener(v -> { chiudiSelettore(); nuovaScheda(HOME_PREDEFINITA, false, false); });
        ImageView incBtn = iconaBtn(R.drawable.ic_incognito); incBtn.setOnClickListener(v -> { chiudiSelettore(); nuovaScheda(HOME_PREDEFINITA, true, false); });
        Button close = iconBtn("✕");  close.setOnClickListener(v -> chiudiSelettore());
        top.addView(title); top.addView(addBtn); top.addView(incBtn); top.addView(close);

        ScrollView sc = new ScrollView(this);
        sc.setLayoutParams(new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f));
        LinearLayout body = new LinearLayout(this);
        body.setOrientation(LinearLayout.VERTICAL);
        body.setPadding(dp(8), 0, dp(8), dp(24));

        int nNorm = 0, nInc = 0; for (Scheda t : schede) { if (t.incognito) nInc++; else nNorm++; }
        if (nNorm > 0) { body.addView(sectionHeader("Schede  (" + nNorm + ")", false)); body.addView(grigliaSchede(false)); }
        if (nInc > 0)  { body.addView(sectionHeader("In incognito  (" + nInc + ")", true)); body.addView(grigliaSchede(true)); }

        sc.addView(body);
        col.addView(top); col.addView(sc);
        overlay.addView(col, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));

        selettore = overlay;
        ((ViewGroup) findViewById(android.R.id.content)).addView(overlay,
            new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
    }

    private void chiudiSelettore() {
        if (selettore != null && selettore.getParent() != null) ((ViewGroup) selettore.getParent()).removeView(selettore);
        selettore = null;
    }
    private void rinfrescaSelettore() { if (selettore != null) { chiudiSelettore(); mostraSelettore(); } }

    private TextView sectionHeader(String text, boolean inc) {
        TextView h = new TextView(this);
        h.setText(text);
        h.setTextColor(inc ? 0xFFb9a7ff : DIM);
        h.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        h.setPadding(dp(8), dp(16), dp(8), dp(6));
        return h;
    }

    private GridLayout grigliaSchede(boolean incognito) {
        GridLayout g = new GridLayout(this);
        g.setColumnCount(2);
        int cardW = (getResources().getDisplayMetrics().widthPixels - dp(16) - dp(24)) / 2;
        int cardH = (int) (cardW * 1.15f);
        for (int i = 0; i < schede.size(); i++) {
            if (schede.get(i).incognito != incognito) continue;
            g.addView(cardScheda(i, cardW, cardH));
        }
        return g;
    }

    private View cardScheda(int index, int cardW, int cardH) {
        final Scheda t = schede.get(index);
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        GridLayout.LayoutParams lp = new GridLayout.LayoutParams();
        lp.width = cardW; lp.height = cardH; lp.setMargins(dp(6), dp(6), dp(6), dp(6));
        card.setLayoutParams(lp);
        GradientDrawable cardBg = new GradientDrawable();
        cardBg.setColor(t.incognito ? INC_CAP : CARD);
        cardBg.setCornerRadius(dp(14));
        if (index == corrente) cardBg.setStroke(dp(2), ACC);
        card.setBackground(cardBg);
        card.setPadding(dp(8), dp(8), dp(8), dp(8));

        // intestazione: titolo + X per chiudere la singola scheda
        LinearLayout head = new LinearLayout(this);
        head.setOrientation(LinearLayout.HORIZONTAL);
        head.setGravity(Gravity.CENTER_VERTICAL);
        TextView tt = new TextView(this);
        tt.setText(t.titolo == null || t.titolo.isEmpty() ? "Nuova scheda" : t.titolo);
        // La casella di una scheda in incognito è scura anche col tema chiaro: il titolo
        // segue la casella, non il tema, altrimenti è testo scuro su viola scuro.
        tt.setTextColor(t.incognito ? INC_TXT : TXT);
        tt.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        tt.setSingleLine(true); tt.setEllipsize(TextUtils.TruncateAt.END);
        tt.setLayoutParams(new LinearLayout.LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f));
        TextView x = new TextView(this);
        x.setText("✕"); x.setTextColor(t.incognito ? INC_DIM : DIM);
        x.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        x.setPadding(dp(8), dp(2), dp(2), dp(2));
        x.setOnClickListener(v -> {
            int idx = schede.indexOf(t);
            if (idx < 0) return;
            boolean ultima = schede.size() == 1;
            chiudiScheda(idx);
            if (!ultima) rinfrescaSelettore();
        });
        head.addView(tt); head.addView(x);

        // anteprima della pagina (bitmap catturata) o placeholder colorato
        ImageView img = new ImageView(this);
        img.setLayoutParams(new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f));
        GradientDrawable ph = new GradientDrawable();
        ph.setColor(t.incognito ? 0xFF241d38 : PH); ph.setCornerRadius(dp(10));
        img.setBackground(ph);
        img.setClipToOutline(true);
        if (t.anteprima != null) {
            // La pagina è una striscia verticale, la casella è più larga che alta: con
            // l'adattamento normale l'anteprima si rimpicciolirebbe fino a entrare in
            // altezza, lasciando metà casella vuota. Qui si ingrandisce quanto basta a
            // riempire la larghezza e si tiene la cima della pagina — che è la parte da
            // cui si riconosce una scheda — tagliando il resto sotto.
            img.setScaleType(ImageView.ScaleType.MATRIX);
            android.graphics.Matrix m = new android.graphics.Matrix();
            float scala = (float) (cardW - dp(16)) / t.anteprima.getWidth();
            m.setScale(scala, scala);
            img.setImageMatrix(m);
            img.setImageBitmap(t.anteprima);
        }

        View gap = new View(this); gap.setLayoutParams(new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, dp(6)));
        card.addView(head); card.addView(gap); card.addView(img);
        card.setOnClickListener(v -> { int idx = schede.indexOf(t); if (idx >= 0) { mostraScheda(idx); chiudiSelettore(); } });
        return card;
    }

    /**
     * Cattura l'anteprima della pagina della scheda.
     *
     * <p>Sotto WebView è una fotografia sincrona: si disegna la vista su una tela. Funziona
     * per la scheda che è a schermo — quella che si sta lasciando o quella corrente — ed è
     * anche il motivo per cui si cattura sempre <b>prima</b> di cambiare scheda: una vista
     * staccata dal contenitore non ha nulla da disegnare.
     *
     * <p>La bitmap che ne esce è grande quanto lo schermo: si rimpicciolisce qui, perché nel
     * selettore si vede a un terzo e tenerla intera significherebbe dieci bitmap grandi
     * quanto lo schermo in memoria.
     */
    private void catturaAnteprima(Scheda t) {
        if (t == null || t.web == null) return;
        try {
            int w = t.web.getWidth(), h = t.web.getHeight();
            if (w <= 0 || h <= 0) return;
            float scala = 0.35f;
            Bitmap bmp = Bitmap.createBitmap(Math.max(1, (int) (w * scala)), Math.max(1, (int) (h * scala)), Bitmap.Config.RGB_565);
            Canvas c = new Canvas(bmp);
            c.drawColor(Color.WHITE);
            c.scale(scala, scala);
            t.web.draw(c);
            t.anteprima = bmp;
        } catch (Exception e) {
            Log.i(TAG, "anteprima non catturata", e);
        }
    }

    // ------------------------------------------------------------------ menu
    private void mostraMenu(View anchor) {
        Scheda s = schedaCorrente();
        boolean bm = s != null && eNeiPreferiti(s.indirizzo);
        boolean indice = s != null && !s.indirizzo.isEmpty();
        boolean indietro = s != null && s.web != null && s.web.canGoBack();
        boolean avanti = s != null && s.web != null && s.web.canGoForward();

        // La palette del pannello segue la scheda in vista: in incognito è quella scura,
        // altrimenti sarebbe un rettangolo chiaro con dentro testo scuro su una barra viola.
        // Il pannello si ricostruisce da zero: i riferimenti alle stelle sono quelli che
        // sta per costruire, non quelli del pannello precedente.
        stellaFila = null; stellaVoceIcona = null; stellaVoceTesto = null;
        boolean inc = s != null && s.incognito;
        pCard = inc ? INC_CARD : CARD;
        pCap  = inc ? INC_CAP  : CAP;
        pTxt  = inc ? INC_TXT  : TXT;
        pDim  = inc ? INC_DIM  : DIM;

        LinearLayout pannello = new LinearLayout(this);
        pannello.setOrientation(LinearLayout.VERTICAL);
        GradientDrawable sfondo = new GradientDrawable();
        sfondo.setColor(pCard);
        sfondo.setCornerRadius(dp(14));
        pannello.setBackground(sfondo);

        // ---- fila di icone: agiscono sulla pagina, non aprono altre schermate ----
        LinearLayout fila = new LinearLayout(this);
        fila.setOrientation(LinearLayout.HORIZONTAL);
        fila.setGravity(Gravity.CENTER_VERTICAL);
        fila.setPadding(dp(6), dp(4), dp(6), dp(4));
        fila.addView(iconaMenu("←", indietro,
                v -> { Scheda c = schedaCorrente(); if (c != null && c.web.canGoBack()) c.web.goBack(); }));
        fila.addView(iconaMenu("→", avanti,
                v -> { Scheda c = schedaCorrente(); if (c != null && c.web.canGoForward()) c.web.goForward(); }));
        stellaFila = iconaMenu(bm ? "★" : "☆", indice, this::invertiPreferitoDaMenu);
        stellaFila.setTextColor(bm ? ACC : pTxt);   // blu quando la pagina è salvata
        fila.addView(stellaFila);
        fila.addView(iconaMenu("⬇", true, v -> apriElencoDownload()));
        fila.addView(iconaMenu("⟳", s != null,
                v -> { Scheda c = schedaCorrente(); if (c != null && c.web != null) c.web.reload(); }));
        pannello.addView(fila);
        pannello.addView(separatore());

        // ---- voci, raggruppate per argomento: le righe di luce sono la sola cosa che dice
        // dove finisce un gruppo e comincia il prossimo ----
        pannello.addView(voceMenu("🦇", nyctalus ? "Esci dalla modalità Nyctalus" : "Modalità Nyctalus (rete anonima)",
                v -> cambiaNyctalus()));
        pannello.addView(separatore());
        pannello.addView(voceMenu("＋", "Nuova scheda",
                v -> nuovaScheda(paginaIniziale(), false, false)));
        pannello.addView(voceMenu(R.drawable.ic_incognito, "Nuova scheda in incognito",
                v -> nuovaScheda(paginaIniziale(), true, false)));
        pannello.addView(separatore());
        pannello.addView(voceMenu(R.drawable.ic_history, "Cronologia", v -> mostraCronologia()));
        pannello.addView(voceMenu("★", "Preferiti", v -> mostraPreferiti()));
        // La voce dei preferiti: si tengono i riferimenti alle sue due parti, perché al
        // tocco la stella deve cambiare aspetto sul posto.
        stellaVoceIcona = glifoMenu(bm ? "★" : "☆");
        stellaVoceIcona.setTextColor(bm ? ACC : pTxt);
        stellaVoceTesto = etichettaMenu(bm ? "Rimuovi dai preferiti" : "Aggiungi ai preferiti");
        pannello.addView(voceMenu(stellaVoceIcona, stellaVoceTesto, this::invertiPreferitoDaMenu));
        pannello.addView(separatore());
        pannello.addView(voceMenu(R.drawable.ic_find, "Trova nella pagina", v -> mostraBarraTrova(true)));
        // «Mostra modalità lettura»: la pagina si legge senza il contorno del sito. Il testo
        // si ricava dalla pagina stessa (v. modalitaLettura) e si apre in una scheda a parte.
        pannello.addView(voceMenu(R.drawable.ic_reader, "Mostra modalità lettura", v -> modalitaLettura()));
        // «Traduci…» cambia nome quando la pagina è già tradotta: la voce dice la prossima
        // mossa, non lo stato — e la stessa cosa fa il pulsante della barra.
        boolean tradotta = s != null && !s.linguaMostrata.isEmpty();
        pannello.addView(voceMenu(R.drawable.ic_translate,
                tradotta ? "Mostra originale" : "Traduci…",
                v -> { Scheda c = schedaCorrente(); if (c == null) return;
                       if (tradotta) mostraOriginale(c); else apriTraduzione(c); }));
        pannello.addView(voceMenu(s != null && s.desktop ? R.drawable.ic_mobile : R.drawable.ic_desktop,
                s != null && s.desktop ? "Sito mobile" : "Sito desktop",
                v -> { Scheda c = schedaCorrente(); if (c != null) cambiaModalita(c, !c.desktop); }));
        pannello.addView(separatore());
        // ---- da una pagina a una cosa che resta: un file, un'icona nella home ----
        pannello.addView(voceMenu(R.drawable.ic_download, "Download", v -> apriElencoDownload()));
        pannello.addView(separatore());
        // ---- quello che si cancella e quello che si concede, tutti e due per sito ----
        pannello.addView(voceMenu(R.drawable.ic_trash, "Elimina cronologia", v -> eliminaCronologia()));
        pannello.addView(voceMenu(R.drawable.ic_settings, "Impostazioni del sito", v -> impostazioniSito()));
        pannello.addView(separatore());
        pannello.addView(voceMenu(R.drawable.ic_fullscreen, "Schermo intero", v -> setSchermoIntero(true)));
        pannello.addView(separatore());
        pannello.addView(voceMenu(R.drawable.ic_share, "Condividi…",
                v -> condividi(schedaCorrente() != null ? schedaCorrente().indirizzo : null)));
        if (!nyctalus)   // in modalità Nyctalus si resterebbe fuori dalla rete senza accorgersene
            pannello.addView(voceMenu(R.drawable.ic_globe, "Apri nel browser di sistema",
                    v -> apriNelBrowserDiSistema(schedaCorrente() != null ? schedaCorrente().indirizzo : null)));
        pannello.addView(separatore());
        pannello.addView(voceMenu(R.drawable.ic_home, "Pagina iniziale", v -> vaiAllaHome()));
        pannello.addView(voceMenu(R.drawable.ic_pin, "Imposta come pagina iniziale", v -> impostaPaginaIniziale()));

        ScrollView contenitore = new ScrollView(this);
        contenitore.addView(pannello);
        // La larghezza del pannello e dove appoggiarlo.
        //
        // Il pannello si aggancia al ⋮, ma il ⋮ è l'ultima icona della barra: appoggiandolo
        // lì si spingerebbe fuori dallo schermo di tutta la sua larghezza. Si aggancia
        // quindi al bordo destro, con un margine, e lo scarto si calcola dalla posizione
        // vera dell'ancora — così vale su qualunque schermo invece che su uno solo.
        int larghezza = Math.min(dp(300), getResources().getDisplayMetrics().widthPixels - dp(16));
        int[] dove = new int[2];
        anchor.getLocationOnScreen(dove);
        int scarto = getResources().getDisplayMetrics().widthPixels - larghezza - dp(8) - dove[0];
        menuAperto = new PopupWindow(contenitore, larghezza, LayoutParams.WRAP_CONTENT, true);
        menuAperto.setBackgroundDrawable(new ColorDrawable(Color.TRANSPARENT));
        menuAperto.setOutsideTouchable(true);
        menuAperto.setElevation(dp(10));
        menuAperto.showAsDropDown(anchor, scarto, dp(6));
    }

    /**
     * Chiude il menu, se è aperto.
     *
     * <p>Il pannello non è un {@link android.widget.PopupMenu}, che si congeda da sé appena
     * una voce è scelta: è una finestra nostra, e una finestra nostra resta aperta finché
     * qualcuno non la chiude. Senza questa riga ogni voce lasciava il menu aperto sopra la
     * schermata che aveva appena aperto — e «Trova nella pagina» finiva sotto le sue stesse
     * voci.
     */
    private void chiudiMenu() {
        if (menuAperto != null) { menuAperto.dismiss(); menuAperto = null; }
    }

    /** Una voce con l'icona disegnata ({@code res/drawable/ic_*.xml}), non un carattere. */
    private View voceMenu(int icona, String testo, View.OnClickListener azione) {
        return voceMenu(iconaVista(icona, 18, pTxt), etichettaMenu(testo), azione);
    }

    /** Il testo di una voce, col colore della palette in uso. */
    private TextView etichettaMenu(String testo) {
        TextView t = new TextView(this);
        t.setText(testo);
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        t.setTextColor(pTxt);
        return t;
    }

    /** Il glifo che fa da icona a una voce, quando non c'è un disegno. */
    private TextView glifoMenu(String glifo) {
        TextView i = new TextView(this);
        i.setText(glifo);
        i.setTextColor(pTxt);
        i.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        i.setGravity(Gravity.CENTER);
        return i;
    }

    /**
     * Una voce con un glifo di testo al posto dell'icona disegnata.
     *
     * <p>Restano a carattere le poche forme che il carattere rende già piene e regolari —
     * la stella, la croce, lo schermo intero, il più — perché sono un tratto solo e non
     * hanno bisogno di un disegno. Le altre (la casa, il lucchetto, gli occhiali, la lente…)
     * sarebbero emoji: a colori, fuori misura e diverse da un telefono all'altro, quindi
     * disegnate.
     */
    private View voceMenu(String glifo, String testo, View.OnClickListener azione) {
        return voceMenu(glifoMenu(glifo), etichettaMenu(testo), azione);
    }

    /**
     * Una voce del menu: icona a sinistra, testo accanto.
     *
     * <p>L'icona sta in una casella di 18 dp seguita da 8 dp di vuoto, invece di essere
     * incollata al testo: le voci si leggono in colonna solo se tutti i testi cominciano
     * alla stessa ascissa, e un glifo e un disegno non sono mai larghi uguale.
     */
    private View voceMenu(View icona, TextView testo, View.OnClickListener azione) {
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.HORIZONTAL);
        r.setGravity(Gravity.CENTER_VERTICAL);
        r.setPadding(dp(16), dp(10), dp(16), dp(10));
        r.setBackgroundResource(sfondoTocco());

        LinearLayout.LayoutParams ilp = new LinearLayout.LayoutParams(dp(18), dp(18));
        ilp.rightMargin = dp(8);
        icona.setLayoutParams(ilp);
        r.addView(icona);
        r.addView(testo);

        r.setOnClickListener(v -> { chiudiMenu(); azione.onClick(v); });
        return r;
    }

    /** Un'icona della fila in alto: cerchio chiaro attorno, spenta se l'azione non c'è. */
    private TextView iconaMenu(String glifo, boolean attiva, View.OnClickListener azione) {
        TextView t = new TextView(this);
        t.setText(glifo);
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, 17);
        t.setTextColor(attiva ? pTxt : pDim);
        t.setGravity(Gravity.CENTER);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(dp(40), dp(40));
        lp.leftMargin = dp(5); lp.rightMargin = dp(5);
        t.setLayoutParams(lp);

        GradientDrawable cerchio = new GradientDrawable();
        cerchio.setShape(GradientDrawable.OVAL);
        cerchio.setColor(attiva ? pCap : Color.TRANSPARENT);
        if (attiva) {
            // il tocco si accende dentro il cerchio, non nel rettangolo attorno
            t.setBackground(new RippleDrawable(ColorStateList.valueOf(0x33808080), cerchio, cerchio));
            t.setOnClickListener(azione);
        } else {
            t.setBackground(cerchio);
        }
        return t;
    }

    /** La riga di luce che separa due gruppi di voci. */
    private View separatore() {
        View v = new View(this);
        v.setBackgroundColor(pDim);
        v.setAlpha(0.22f);
        v.setLayoutParams(new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, Math.max(1, dp(1) / 2)));
        return v;
    }

    /** Lo sfondo che si accende al tocco: è il «ripple» del tema, non un colore nostro. */
    private int sfondoTocco() {
        TypedValue v = new TypedValue();
        getTheme().resolveAttribute(android.R.attr.selectableItemBackground, v, true);
        return v.resourceId;
    }

    /**
     * L'elenco dei download, dentro il browser.
     *
     * <p>Prima questa voce passava all'app dei download del sistema: i file si vedevano, ma
     * la via d'uscita da quella schermata dipendeva da quella app — e su un telefono può non
     * esserci, lasciando il browser irraggiungibile. L'elenco lo disegna quindi il browser,
     * con la stessa domanda al sistema che fa quell'app ({@link DownloadManager}) e con un
     * solo modo di chiuderlo: il suo pulsante. I file restano dove sono — la cartella
     * Download del telefono — e da qui si aprono con l'app che li sa leggere.
     */
    private void apriElencoDownload() {
        DownloadManager dm = (DownloadManager) getSystemService(Context.DOWNLOAD_SERVICE);
        if (dm == null) { Toast.makeText(this, "Download non disponibile", Toast.LENGTH_SHORT).show(); return; }

        final List<Scaricato> righe = new ArrayList<>();
        android.database.Cursor c = null;
        try {
            // Tutti i download del telefono, compresi quelli in corso: un elenco che
            // mostrasse solo i file finiti sembrerebbe non aver visto il download appena
            // avviato. I valori si leggono per nome di colonna — gli indici fissi cambiano
            // da una versione all'altra — e l'ordine si fa qui sotto: la Query di questa
            // versione di Android non ha un «ordina per», quindi il sistema non lo fa.
            c = dm.query(new DownloadManager.Query());
            while (c != null && c.moveToNext()) {
                righe.add(new Scaricato(
                        c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_ID)),
                        c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_LAST_MODIFIED_TIMESTAMP)),
                        c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_TOTAL_SIZE_BYTES)),
                        c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS)),
                        c.getString(c.getColumnIndexOrThrow(DownloadManager.COLUMN_TITLE)),
                        c.getString(c.getColumnIndexOrThrow(DownloadManager.COLUMN_MEDIA_TYPE))));
            }
        } catch (Exception e) {
            Log.w(TAG, "elenco download non leggibile", e);
        } finally {
            if (c != null) c.close();
        }
        righe.sort((x, y) -> Long.compare(y.quando, x.quando));

        if (righe.isEmpty()) {
            mostraAvviso("Download",
                "Nessun download.\nI file scaricati dalle pagine finiscono nella cartella Download del telefono: li trovi anche da lì.",
                null, Pulsante.principale("Chiudi", null));
            return;
        }
        LinearLayout corpo = apriFoglio(righe.size() == 1 ? "Download" : "Download (" + righe.size() + ")", "Chiudi");
        for (final Scaricato s : righe) {
            corpo.addView(rigaFoglio(s.nome(), s.dettaglio(), () -> apriFileScaricato(s)));
        }
    }

    /** Una voce dell'elenco dei download: quello che serve a mostrarla e ad aprirla. */
    private static class Scaricato {
        final long id, quando, quanti;
        final int stato;
        final String nome, mime;
        Scaricato(long id, long quando, long quanti, int stato, String nome, String mime) {
            this.id = id; this.quando = quando; this.quanti = quanti; this.stato = stato;
            this.nome = nome == null || nome.isEmpty() ? "File" : nome;
            this.mime = mime == null ? "" : mime;
        }
        String nome() { return nome; }
        String dettaglio() {
            StringBuilder b = new StringBuilder();
            if (quanti > 0) b.append(dimensione(quanti));
            if (stato != DownloadManager.STATUS_SUCCESSFUL) {
                if (b.length() > 0) b.append(" · ");
                b.append(statoDownload(stato));
            }
            return b.toString();
        }
    }

    /** Apre un file scaricato con l'app che lo sa leggere. */
    private void apriFileScaricato(Scaricato s) {
        if (s.stato != DownloadManager.STATUS_SUCCESSFUL) {
            Toast.makeText(this, statoDownload(s.stato), Toast.LENGTH_SHORT).show();
            return;
        }
        try {
            DownloadManager dm = (DownloadManager) getSystemService(Context.DOWNLOAD_SERVICE);
            Uri u = dm == null ? null : dm.getUriForDownloadedFile(s.id);
            if (u == null) { Toast.makeText(this, "Il file non c'è più", Toast.LENGTH_SHORT).show(); return; }
            Intent i = new Intent(Intent.ACTION_VIEW);
            i.setDataAndType(u, s.mime.isEmpty() ? "*/*" : s.mime);
            i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            startActivity(i);
        } catch (Exception e) {
            // Nessuna app installata sa leggere questo tipo di file: è una risposta, non un guasto.
            Log.w(TAG, "nessuna app per questo file", e);
            Toast.makeText(this, "Nessuna app per aprire questo file", Toast.LENGTH_SHORT).show();
        }
    }

    private static String statoDownload(int stato) {
        switch (stato) {
            case DownloadManager.STATUS_PENDING: return "in attesa";
            case DownloadManager.STATUS_RUNNING: return "in corso";
            case DownloadManager.STATUS_PAUSED: return "in pausa";
            case DownloadManager.STATUS_FAILED: return "non riuscito";
            default: return "";
        }
    }

    /** Una dimensione leggibile: «412 B», «38 kB», «1,2 MB». */
    private static String dimensione(long b) {
        if (b < 1024) return b + " B";
        if (b < 1024L * 1024L) return String.format(Locale.ITALY, "%.0f kB", b / 1024.0);
        return String.format(Locale.ITALY, "%.1f MB", b / (1024.0 * 1024.0));
    }

    /**
     * Passa una scheda da sito mobile a sito desktop e viceversa.
     *
     * <p>Sotto WebView cambia una cosa sola — la stringa del browser — perché la larghezza
     * della vista la decide il sito leggendo quella. Sotto Gecko ne cambiano due (la stringa
     * e la larghezza dichiarata), e non è una differenza di stile: è il motivo per cui la
     * stessa voce è scritta in due modi nei due file.
     */
    private void cambiaModalita(Scheda s, boolean desktop) {
        s.desktop = desktop;
        // La scelta vale per il sito e non per la scheda: chi ha bisogno della vista desktop
        // su un sito la vuole tutte le volte, anche nella scheda aperta domani. Ed è la
        // stessa voce che si trova nelle impostazioni del sito.
        decidi(hostDi(s.indirizzo), "desktop", desktop ? "si" : "no");
        applicaUa(s);
        if (s.web != null) s.web.reload();
    }

    private void applicaUa(Scheda t) {
        if (t == null || t.web == null) return;
        WebSettings s = t.web.getSettings();
        if (mobileUa == null) mobileUa = s.getUserAgentString() + " NovaOS";
        if (t.desktop) s.setUserAgentString(UA_DESKTOP);
        else s.setUserAgentString(mobileUa);
        // La vista desktop si dichiara anche alla pagina: senza, il sito si impagina da
        // telefono anche con la stringa da computer.
        s.setUseWideViewPort(true);
        s.setLoadWithOverviewMode(true);
    }

    /**
     * Se questo sito va mostrato da computer.
     *
     * <p>L'ordine è: la scelta dell'utente, se c'è; poi i due siti che da telefono non
     * funzionano (sono applicazioni da scrivania, e in vista mobile si aprono a metà); poi
     * no. La scelta dell'utente viene prima di tutto perché è l'unica che sa qualcosa che
     * noi non sappiamo.
     */
    private boolean vuoleDesktop(String url) {
        String host = hostDi(url);
        if (host.isEmpty()) return false;
        String scelta = decisione(host, "desktop");
        if ("si".equals(scelta)) return true;
        if ("no".equals(scelta)) return false;
        return host.contains("web.whatsapp.com") || host.contains("web.telegram.org");
    }

    // ------------------------------------------------------------------ navigazione
    private void naviga(String input) {
        input = input == null ? "" : input.trim();
        if (input.isEmpty()) return;
        String url;
        if (input.matches("^[a-zA-Z][a-zA-Z0-9+.\\-]*://.*")) url = input;
        else if (input.toLowerCase(Locale.ROOT).matches("^[a-z2-7]{56}\\.nyct(/.*)?$")) url = "http://" + input;
        else if (input.contains(".") && !input.contains(" ")) url = "https://" + input;
        else url = (nyctalus ? SEARCH_NYCT : SEARCH) + Uri.encode(input);
        if (eNyct(url) && !nyctalus) { attivaNyctalus(url); hideKeyboard(); return; }
        Scheda s = schedaCorrente();
        if (s != null && s.web != null) s.web.loadUrl(url);
        hideKeyboard();
        omnibox.clearFocus();
    }

    private void syncBar(String url) {
        if (url == null) url = "";
        if (!omnibox.hasFocus()) omnibox.setText(url);
        Scheda s = schedaCorrente();
        boolean inc = s != null && s.incognito;
        boolean sicura = url.startsWith("https://");
        secIco.setImageResource(sicura ? R.drawable.ic_lock : R.drawable.ic_info);
        // Il lucchetto segue la palette della barra: in incognito quella chiara del tema
        // sarebbe invisibile sul viola scuro.
        secIco.setColorFilter(sicura ? (inc ? INC_DIM : DIM) : 0xFFff9f0a, PorterDuff.Mode.SRC_IN);
        applicaTemaBarra(inc);
    }

    /** La scheda in vista è in incognito? */
    private boolean incognitoInVista() {
        Scheda s = schedaCorrente();
        return s != null && s.incognito;
    }

    /**
     * Colora la barra secondo la palette in uso: quella di NovaOS, oppure quella
     * dell'incognito (barra scura viola).
     *
     * <p>Non è solo lo sfondo: in incognito cambiano <b>anche i testi e le icone</b>, perché
     * la barra è scura comunque mentre il tema di NovaOS può essere chiaro. Tingere solo lo
     * sfondo lasciava il testo nero su viola scuro — si vedeva la barra e non quello che
     * c'era scritto.
     */
    private void applicaTemaBarra(boolean inc) {
        int cBar = inc ? INC_BAR : BAR;
        int cCap = inc ? INC_CAP : CAP;
        int cTxt = inc ? INC_TXT : TXT;
        int cDim = inc ? INC_DIM : DIM;

        bar.setBackgroundColor(cBar);
        GradientDrawable capBg = new GradientDrawable();
        capBg.setColor(cCap); capBg.setCornerRadius(dp(22));
        cap.setBackground(capBg);
        omnibox.setTextColor(cTxt);
        omnibox.setHintTextColor(cDim);
        omnibox.setHint(inc ? "Cerca o digita (incognito)" : "Cerca o digita un indirizzo");
        incBadge.setVisibility(inc ? View.VISIBLE : View.GONE);
        tabBtn.setTextColor(cTxt);
        tabBtn.setBackground(squareBadge(cDim));
        for (TextView t : testiTema) t.setTextColor(cTxt);
        for (ImageView i : iconeTema) i.setColorFilter(cTxt, PorterDuff.Mode.SRC_IN);

        // La barra «trova» è la seconda riga della stessa testata, e la riga di
        // avanzamento ne è il bordo inferiore: seguono la stessa palette.
        findBar.setBackgroundColor(cBar);
        findInfo.setTextColor(cDim);
        findInput.setTextColor(cTxt);
        findInput.setHintTextColor(cDim);
        progress.setProgressBackgroundTintList(ColorStateList.valueOf(cBar));

        // La stella dei preferiti ha un colore suo (blu quando la pagina è salvata) che
        // non è né quello del testo né quello delle icone: si ricalcola qui.
        aggiornaStellaUI();
    }

    // ================================================================== il motore
    // Una WebView per scheda, con i suoi due client. I client sono scritti dentro
    // buildWebView perché devono sapere a quale scheda appartengono: le notifiche arrivano
    // anche per le schede in secondo piano (una pagina di sotto continua a caricare) e
    // vanno scritte nella scheda giusta, non in quella in vista.

    private WebView buildWebView(final Scheda tab) {
        WebView v = new WebView(this);
        WebSettings s = v.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(!tab.incognito);
        s.setDatabaseEnabled(!tab.incognito);
        s.setSupportZoom(true);
        s.setBuiltInZoomControls(true);
        s.setDisplayZoomControls(false);
        s.setUseWideViewPort(true);
        s.setLoadWithOverviewMode(true);
        s.setMixedContentMode(WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE);
        // I popup: senza queste due, window.open e i «accedi con…» non aprono nulla e la
        // pagina resta in attesa di una finestra che non compare.
        s.setSupportMultipleWindows(true);
        s.setJavaScriptCanOpenWindowsAutomatically(true);
        // Geolocalizzazione: la WebView la chiede solo se è accesa qui.
        s.setGeolocationEnabled(!nyctalus);
        if (nyctalus && !ProxyNyctalus.scriptAllAvvio(v, JS_NO_WEBRTC)) Log.w(TAG, "script all'avvio non disponibile: WebRTC spento a pagina iniziata");
        if (tab.incognito) s.setCacheMode(WebSettings.LOAD_NO_CACHE);
        applicaUa(tab);

        CookieManager cm = CookieManager.getInstance();
        cm.setAcceptCookie(true);
        // Di serie la WebView non manda i cookie di terze parti, e senza di quelli i
        // «accedi con…» e i carrelli si rompono: si accettano per la navigazione normale, e
        // non in incognito, dove il senso è non lasciare traccia.
        cm.setAcceptThirdPartyCookies(v, !tab.incognito);

        v.setWebViewClient(new WebViewClient() {
            @Override public void onPageStarted(WebView vw, String url, Bitmap f) {
                if (nyctalus) vw.evaluateJavascript(JS_NO_WEBRTC, null);   // (doppia sicurezza)
                tab.indirizzo = url == null ? "" : url;
                // Una pagina nuova è una pagina non tradotta: la barra sparisce adesso e
                // ricompare, se serve, a caricamento finito. Lasciarla su vorrebbe dire
                // offrire la traduzione di un testo che a video non c'è ancora.
                tab.linguaPagina = "";
                tab.linguaMostrata = "";
                tab.blocchiTradotti = null;
                tab.linguaBlocchi = "";
                if (vw == webCorrente()) { mostraBarraTraduzione(false); syncBar(tab.indirizzo); }
            }
            @Override public void onPageFinished(WebView vw, String url) {
                tab.indirizzo = url == null ? "" : url;
                if (tab.titolo == null || tab.titolo.isEmpty()) {
                    String t = vw.getTitle();
                    if (t != null && !t.isEmpty()) tab.titolo = t;
                }
                if (vw == webCorrente()) syncBar(tab.indirizzo);
                // La lingua si legge solo per la scheda in vista: la barra riguarda quella,
                // e chiederla a tutte le schede a ogni caricamento è lavoro sprecato.
                if (vw == webCorrente()) leggiLinguaPagina(tab);
                // La cronologia si scrive a caricamento finito: a pagina iniziata il titolo
                // non c'è ancora, e le voci sarebbero tutte senza nome.
                // Una scheda di lettura non è una pagina visitata: l'indirizzo è quello del
                // sito, ma quello che si è letto è il testo che ne abbiamo ricavato.
                if (!tab.incognito && !tab.lettura && !nyctalus) addCronologia(url, tab.titolo);
            }
            @Override public boolean shouldOverrideUrlLoading(WebView vw, WebResourceRequest req) {
                String u = req.getUrl() != null ? req.getUrl().toString() : "";
                // .nyct: solo dentro la rete (fuori finirebbe in chiaro a chi sa chi)
                if (eNyct(u) && !nyctalus) {
                    Toast.makeText(BrowserActivity.this, "I siti .nyct si aprono solo in modalità Nyctalus (🦇)", Toast.LENGTH_LONG).show();
                    return true;
                }
                // in modalità Nyctalus nessun'altra app riceve indirizzi (uscirebbero dalla rete)
                if (nyctalus && !u.startsWith("http://") && !u.startsWith("https://") && !u.startsWith("about:")
                        && !u.startsWith("data:") && !u.startsWith("blob:")) {
                    Toast.makeText(BrowserActivity.this, "Bloccato in modalità Nyctalus: " + Uri.parse(u).getScheme() + ":", Toast.LENGTH_SHORT).show();
                    return true;
                }
                // schemi non http (tel:, mailto:, intent:) -> delega al sistema
                if (!u.startsWith("http://") && !u.startsWith("https://") && !u.startsWith("about:") && !u.startsWith("data:")) {
                    try { startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(u))); } catch (Exception e) {
                        Log.i(TAG, "schema non gestito da nessuna app: " + u, e);
                    }
                    return true;
                }
                return false;
            }
            /** Fuori dalla modalità Nyctalus nessuna richiesta verso .nyct, nemmeno dalle pagine. */
            @Override public android.webkit.WebResourceResponse shouldInterceptRequest(WebView vw, WebResourceRequest req) {
                if (!nyctalus && req != null && req.getUrl() != null && eNyct(req.getUrl().toString())) {
                    return new android.webkit.WebResourceResponse("text/plain", "utf-8", 403, "Bloccato",
                        new java.util.HashMap<String, String>(),
                        new java.io.ByteArrayInputStream("I siti .nyct si aprono solo in modalità Nyctalus".getBytes()));
                }
                return null;
            }
            @Override public void onReceivedError(WebView vw, WebResourceRequest req, WebResourceError err) {
                if (req == null || !req.isForMainFrame()) return;
                Log.w(TAG, "caricamento fallito: " + req.getUrl());
                tab.titolo = "Impossibile caricare la pagina";
                if (vw == webCorrente()) {
                    progress.setVisibility(View.GONE);
                    Toast.makeText(BrowserActivity.this, "Impossibile caricare la pagina", Toast.LENGTH_SHORT).show();
                }
            }
            /**
             * Il processo di disegno delle pagine è morto: la scheda resta, la pagina no.
             *
             * <p>Senza questa risposta la schermata resta bianca e sembra un blocco
             * dell'app, mentre la WebView è già inerte. Si ricarica e si dice cosa è
             * successo — è la stessa scelta che il browser fa sotto Gecko.
             */
            @Override public boolean onRenderProcessGone(WebView vw, android.webkit.RenderProcessGoneDetail dettaglio) {
                Log.e(TAG, "il processo di disegno è andato in crash");
                Toast.makeText(BrowserActivity.this, "La pagina è andata in crash: la ricarico", Toast.LENGTH_SHORT).show();
                int idx = schede.indexOf(tab);
                if (idx >= 0) chiudiScheda(idx);
                return true;   // l'abbiamo gestito noi: la vista morente non va più toccata
            }
        });
        v.setWebChromeClient(new WebChromeClient() {
            @Override public void onProgressChanged(WebView vw, int p) {
                if (vw != webCorrente()) return;
                if (p < 100) { progress.setVisibility(View.VISIBLE); progress.setProgress(p); }
                else progress.setVisibility(View.GONE);
            }
            @Override public void onReceivedTitle(WebView vw, String title) {
                if (title != null && !title.isEmpty()) tab.titolo = title;
            }
            /** {@code window.open} e i popup: si aprono in una scheda nuova. */
            @Override public boolean onCreateWindow(WebView vw, boolean dialog, boolean gesture, Message resultMsg) {
                // Se per questo sito i pop-up sono bloccati (v. «Impostazioni del sito»), la
                // finestra non si apre: restituire false lascia la richiesta cadere, ed è
                // esattamente quello che deve succedere.
                String sito = hostDi(tab.indirizzo);
                if ("block".equals(decisione(sito, "popup"))) {
                    Toast.makeText(BrowserActivity.this, "Pop-up bloccato: " + sito, Toast.LENGTH_SHORT).show();
                    return false;
                }
                Scheda nuova = nuovaScheda(null, tab.incognito, tab.desktop);
                if (resultMsg != null && resultMsg.obj instanceof WebView.WebViewTransport && nuova.web != null) {
                    ((WebView.WebViewTransport) resultMsg.obj).setWebView(nuova.web);
                    resultMsg.sendToTarget();
                    return true;
                }
                return false;
            }
            /** {@code window.close()}: si chiude la scheda che lo ha chiesto. */
            @Override public void onCloseWindow(WebView vw) {
                int idx = schede.indexOf(tab);
                if (idx >= 0) chiudiScheda(idx);
            }

            // ---- le finestrelle della pagina -------------------------------------
            // Sotto WebView funzionano anche da sole, ma mostrerebbero i dialoghi di
            // sistema, diversi da tutto il resto di NovaOS: si disegnano qui, con lo stesso
            // aspetto del browser. Restituire false lascerebbe il dialogo predefinito.
            @Override public boolean onJsAlert(WebView vw, String url, String messaggio, final android.webkit.JsResult esito) {
                new AlertDialog.Builder(BrowserActivity.this, AlertDialog.THEME_DEVICE_DEFAULT_DARK)
                    .setTitle("La pagina dice")
                    .setMessage(messaggio == null ? "" : messaggio)
                    .setPositiveButton("OK", (d, w) -> esito.confirm())
                    .setOnCancelListener(d -> esito.cancel())
                    .show();
                return true;
            }
            @Override public boolean onJsConfirm(WebView vw, String url, String messaggio, final android.webkit.JsResult esito) {
                new AlertDialog.Builder(BrowserActivity.this, AlertDialog.THEME_DEVICE_DEFAULT_DARK)
                    .setTitle("La pagina dice")
                    .setMessage(messaggio == null ? "" : messaggio)
                    .setPositiveButton("OK", (d, w) -> esito.confirm())
                    .setNegativeButton("Annulla", (d, w) -> esito.cancel())
                    .setOnCancelListener(d -> esito.cancel())
                    .show();
                return true;
            }
            @Override public boolean onJsPrompt(WebView vw, String url, String messaggio, String predefinito, final android.webkit.JsPromptResult esito) {
                final EditText campo = new EditText(BrowserActivity.this);
                campo.setText(predefinito == null ? "" : predefinito);
                campo.setSelectAllOnFocus(true);
                int pad = dp(18);
                FrameLayout box = new FrameLayout(BrowserActivity.this);
                box.setPadding(pad, pad / 2, pad, 0);
                box.addView(campo);
                new AlertDialog.Builder(BrowserActivity.this, AlertDialog.THEME_DEVICE_DEFAULT_DARK)
                    .setTitle("La pagina dice")
                    .setMessage(messaggio == null ? "" : messaggio)
                    .setView(box)
                    .setPositiveButton("OK", (d, w) -> esito.confirm(campo.getText().toString()))
                    .setNegativeButton("Annulla", (d, w) -> esito.cancel())
                    .setOnCancelListener(d -> esito.cancel())
                    .show();
                return true;
            }
            /** «Vuoi davvero uscire?» di un modulo compilato: la risposta cauta è «resta». */
            @Override public boolean onJsBeforeUnload(WebView vw, String url, String messaggio, final android.webkit.JsResult esito) {
                new AlertDialog.Builder(BrowserActivity.this, AlertDialog.THEME_DEVICE_DEFAULT_DARK)
                    .setTitle("Uscire dalla pagina?")
                    .setMessage("I dati inseriti nel modulo potrebbero andare persi.")
                    .setPositiveButton("Esci", (d, w) -> esito.confirm())
                    .setNegativeButton("Resta", (d, w) -> esito.cancel())
                    .setOnCancelListener(d -> esito.cancel())
                    .show();
                return true;
            }

            /** Un allegato da scegliere: si apre la schermata di scelta file di Android. */
            @Override public boolean onShowFileChooser(WebView vw, android.webkit.ValueCallback<Uri[]> cb, FileChooserParams params) {
                if (fileCallback != null) { fileCallback.onReceiveValue(null); }
                fileCallback = cb;
                Intent intent;
                try { intent = params.createIntent(); }
                catch (Exception e) {
                    intent = new Intent(Intent.ACTION_GET_CONTENT);
                    intent.setType("*/*"); intent.addCategory(Intent.CATEGORY_OPENABLE);
                }
                if (params.getMode() == FileChooserParams.MODE_OPEN_MULTIPLE) intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
                try { startActivityForResult(Intent.createChooser(intent, "Seleziona"), RICHIESTA_FILE); }
                catch (Exception e) {
                    Log.w(TAG, "nessuna app per scegliere un file", e);
                    fileCallback = null;
                    return false;
                }
                return true;
            }

            /**
             * La pagina chiede la fotocamera o il microfono ({@code getUserMedia}).
             *
             * <p>La regola è: prima Android, poi la pagina. Concedere alla pagina un
             * permesso che Android non ha dato significa una fotocamera che non inquadra —
             * cioè di nuovo un guasto che non somiglia a un guasto.
             */
            @Override public void onPermissionRequest(final PermissionRequest richiesta) {
                if (nyctalus) { richiesta.deny(); return; }   // niente fotocamera/microfono in rete
                final String[] risorse = richiesta.getResources();
                boolean vuoleVideo = false, vuoleAudio = false;
                for (String r : risorse) {
                    if (PermissionRequest.RESOURCE_VIDEO_CAPTURE.equals(r)) vuoleVideo = true;
                    if (PermissionRequest.RESOURCE_AUDIO_CAPTURE.equals(r)) vuoleAudio = true;
                }
                ArrayList<String> concesse = new ArrayList<>();
                // Bloccate nelle impostazioni del sito: la pagina non le riceve e non si
                // chiede niente ad Android. È l'unica cosa che possiamo promettere su questi
                // permessi — «consentire in anticipo» no, perché la fotocamera senza il
                // permesso di Android inquadrerebbe il vuoto.
                String sito = richiesta.getOrigin() == null ? "" : richiesta.getOrigin().getHost();
                if ("block".equals(decisione(sito, "camera"))) vuoleVideo = false;
                if ("block".equals(decisione(sito, "microfono"))) vuoleAudio = false;
                if (!vuoleVideo && !vuoleAudio) { richiesta.deny(); return; }
                if (vuoleVideo && checkSelfPermission(android.Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED)
                    concesse.add(PermissionRequest.RESOURCE_VIDEO_CAPTURE);
                if (vuoleAudio && checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED)
                    concesse.add(PermissionRequest.RESOURCE_AUDIO_CAPTURE);
                if (!concesse.isEmpty()) { richiesta.grant(concesse.toArray(new String[0])); return; }

                final ArrayList<String> mancanti = new ArrayList<>();
                if (vuoleVideo && checkSelfPermission(android.Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED)
                    mancanti.add(android.Manifest.permission.CAMERA);
                if (vuoleAudio && checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED)
                    mancanti.add(android.Manifest.permission.RECORD_AUDIO);
                if (mancanti.isEmpty()) { richiesta.deny(); return; }

                if (mediaInAttesa != null) {
                    // Una sola richiesta media per volta: rispondere più tardi a una pagina
                    // che nel frattempo ha rinunciato non serve a nessuno. Meglio un «no».
                    richiesta.deny();
                    return;
                }
                mediaInAttesa = richiesta;
                mediaVuoleVideo = vuoleVideo;
                mediaVuoleAudio = vuoleAudio;
                unPermessoAllaVolta(() -> requestPermissions(mancanti.toArray(new String[0]), RICHIESTA_MEDIA));
            }

            /** La pagina chiede di sapere dove sei: si chiede la posizione ad Android. */
            @Override public void onGeolocationPermissionsShowPrompt(String origine, final GeolocationPermissions.Callback cb) {
                if (nyctalus || "block".equals(decisione(hostDi(origine), "posizione"))) { cb.invoke(origine, false, false); return; }
                if (checkSelfPermission(android.Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED) {
                    cb.invoke(origine, true, false);
                    return;
                }
                posizioneInAttesa = cb;
                origineInAttesa = origine;
                unPermessoAllaVolta(() -> requestPermissions(new String[]{
                        android.Manifest.permission.ACCESS_FINE_LOCATION,
                        android.Manifest.permission.ACCESS_COARSE_LOCATION }, RICHIESTA_POSIZIONE));
            }

            @Override public void onGeolocationPermissionsHidePrompt() {
                if (posizioneInAttesa != null) {
                    posizioneInAttesa.invoke(origineInAttesa, false, false);
                    posizioneInAttesa = null;
                }
            }
        });

        // download reali (PDF, immagini, file): usa il DownloadManager di sistema.
        // Il limite è che qui arriva un indirizzo e non lo stream: il file lo scarica il
        // sistema, senza i cookie della pagina, quindi un PDF dietro login non si scarica.
        v.setDownloadListener((url, ua, disp, mime, len) -> {
            if (nyctalus) { scaricaInRete(url, ua, disp, mime); return; }
            try {
                DownloadManager.Request r = new DownloadManager.Request(Uri.parse(url));
                r.setMimeType(mime);
                r.addRequestHeader("User-Agent", ua);
                r.addRequestHeader("Cookie", CookieManager.getInstance().getCookie(url));
                r.setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED);
                String name = URLUtil.guessFileName(url, disp, mime);
                r.setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, name);
                DownloadManager dm = (DownloadManager) getSystemService(Context.DOWNLOAD_SERVICE);
                dm.enqueue(r);
                Toast.makeText(BrowserActivity.this, "Download avviato: " + name, Toast.LENGTH_SHORT).show();
            } catch (Exception e) {
                Log.e(TAG, "download non riuscito", e);
                Toast.makeText(BrowserActivity.this, "Download non riuscito", Toast.LENGTH_SHORT).show();
            }
        });

        // Nessun setFindListener: la WebView ne ha uno, ma non è utilizzabile — v.
        // contaOccorrenze(), che il conteggio lo chiede alla pagina.
        return v;
    }

    private WebView webCorrente() { Scheda s = schedaCorrente(); return s == null ? null : s.web; }

    /**
     * Download in modalità Nyctalus: il gestore di sistema scaricherebbe FUORI dalla rete,
     * quindi si scarica qui, attraverso Nyctalus, nella cartella dell'app (Download).
     */
    private void scaricaInRete(final String url, final String ua, final String disp, final String mime) {
        final String nome = URLUtil.guessFileName(url, disp, mime).replaceAll("[\\\\/:*?\"<>|]", "_");
        Toast.makeText(this, "Scarico attraverso Nyctalus: " + nome, Toast.LENGTH_SHORT).show();
        new Thread(() -> {
            String esito;
            HttpURLConnection c = null;
            try {
                File cartella = getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS);
                if (cartella == null) cartella = new File(getFilesDir(), "Download");
                cartella.mkdirs();
                File dest = new File(cartella, nome);
                c = (HttpURLConnection) apri(new URL(url));
                c.setRequestProperty("User-Agent", ua);
                String cookie = CookieManager.getInstance().getCookie(url);
                if (cookie != null) c.setRequestProperty("Cookie", cookie);
                c.setConnectTimeout(30000);
                c.setReadTimeout(60000);
                try (InputStream in = c.getInputStream(); FileOutputStream out = new FileOutputStream(dest)) {
                    byte[] buf = new byte[65536];
                    int n;
                    while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
                }
                esito = "Scaricato: " + dest.getAbsolutePath();
            } catch (Exception e) {
                esito = "Download non riuscito: " + e.getMessage();
            } finally {
                if (c != null) c.disconnect();
            }
            final String msg = esito;
            runOnUiThread(() -> Toast.makeText(this, msg, Toast.LENGTH_LONG).show());
        }).start();
    }

    // ------------------------------------------------------------------ permessi (una alla volta)
    private static final int RICHIESTA_MEDIA = 4811;
    private static final int RICHIESTA_POSIZIONE = 4812;
    private static final int RICHIESTA_FILE = 4801;

    private android.webkit.ValueCallback<Uri[]> fileCallback;
    private PermissionRequest mediaInAttesa;
    private boolean mediaVuoleVideo, mediaVuoleAudio;
    private GeolocationPermissions.Callback posizioneInAttesa;
    private String origineInAttesa;

    /**
     * Una richiesta di permesso per volta.
     *
     * <p>Android accetta un solo dialogo di permessi alla volta, e la seconda richiesta
     * aperta mentre la prima è in corso viene <b>rifiutata all'istante</b> — un rifiuto
     * tecnico indistinguibile da un «no» dell'utente. Il caso non è teorico: una pagina che
     * chiama {@code getUserMedia({audio:true, video:true})} fa passare di qui due permessi
     * insieme, e basta un secondo tocco perché la coda diventi necessaria.
     */
    private boolean permessiInCorso = false;
    private final ArrayDeque<Runnable> permessiInCoda = new ArrayDeque<>();

    private void unPermessoAllaVolta(Runnable richiesta) {
        if (permessiInCorso) { permessiInCoda.add(richiesta); return; }
        permessiInCorso = true;
        richiesta.run();
    }

    private void turnoSuccessivoDeiPermessi() {
        Runnable prossima = permessiInCoda.poll();
        if (prossima == null) { permessiInCorso = false; return; }
        prossima.run();
    }

    @Override public void onRequestPermissionsResult(int codice, String[] permessi, int[] esiti) {
        super.onRequestPermissionsResult(codice, permessi, esiti);
        boolean tutti = esiti.length > 0;
        for (int e : esiti) if (e != PackageManager.PERMISSION_GRANTED) tutti = false;
        if (codice == RICHIESTA_MEDIA) {
            PermissionRequest richiesta = mediaInAttesa;
            boolean video = mediaVuoleVideo, audio = mediaVuoleAudio;
            mediaInAttesa = null;
            if (richiesta != null) {
                ArrayList<String> concesse = new ArrayList<>();
                if (video && tutti) concesse.add(PermissionRequest.RESOURCE_VIDEO_CAPTURE);
                if (audio && tutti) concesse.add(PermissionRequest.RESOURCE_AUDIO_CAPTURE);
                if (concesse.isEmpty()) richiesta.deny();
                else richiesta.grant(concesse.toArray(new String[0]));
            }
        } else if (codice == RICHIESTA_POSIZIONE) {
            GeolocationPermissions.Callback cb = posizioneInAttesa;
            String origine = origineInAttesa;
            posizioneInAttesa = null;
            if (cb != null) cb.invoke(origine, tutti, false);
        }
        turnoSuccessivoDeiPermessi();
    }

    @Override protected void onActivityResult(int codice, int esito, Intent dati) {
        super.onActivityResult(codice, esito, dati);
        if (codice != RICHIESTA_FILE) return;
        android.webkit.ValueCallback<Uri[]> cb = fileCallback;
        fileCallback = null;
        if (cb == null) return;
        cb.onReceiveValue(WebChromeClient.FileChooserParams.parseResult(esito, dati));
    }

    // ------------------------------------------------------------------ trova nella pagina
    private LinearLayout buildFindBar() {
        LinearLayout f = new LinearLayout(this);
        f.setOrientation(LinearLayout.HORIZONTAL);
        f.setGravity(Gravity.CENTER_VERTICAL);
        f.setBackgroundColor(BAR);
        f.setPadding(dp(12), dp(6), dp(8), dp(6));
        f.setVisibility(View.GONE);
        findInput = new EditText(this);
        findInput.setSingleLine(true);
        findInput.setHint("Trova nella pagina");
        findInput.setHintTextColor(DIM);
        findInput.setTextColor(TXT);
        findInput.setBackgroundColor(Color.TRANSPARENT);
        findInput.setLayoutParams(new LinearLayout.LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f));
        findInput.addTextChangedListener(new TextWatcher() {
            public void beforeTextChanged(CharSequence s, int a, int b, int c) {}
            public void onTextChanged(CharSequence s, int a, int b, int c) { cerca(s.toString()); }
            public void afterTextChanged(Editable s) {}
        });
        findInfo = new TextView(this);
        findInfo.setTextColor(DIM);
        findInfo.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        findInfo.setPadding(dp(6), 0, dp(6), 0);
        Button prev = iconBtn("↑"); prev.setOnClickListener(v -> salta(false));
        Button next = iconBtn("↓"); next.setOnClickListener(v -> salta(true));
        Button x = iconBtn("✕"); x.setOnClickListener(v -> mostraBarraTrova(false));
        // La barra «trova» sta sotto quella superiore e ne segue i colori: in incognito
        // deve essere scura come lei, altrimenti è una striscia chiara in mezzo al viola.
        testiTema.add(prev); testiTema.add(next); testiTema.add(x);
        f.addView(findInput); f.addView(findInfo); f.addView(prev); f.addView(next); f.addView(x);
        return f;
    }

    /**
     * Cerca e mette in evidenza tutte le occorrenze, e ne fa contare quante sono.
     *
     * <p>L'evidenziazione e il salto sono della WebView, e funzionano: si vedono le
     * occorrenze accendersi e il salto che le percorre. Il <b>numero</b> invece no — e non
     * è una svista, è stato misurato. Con due occorrenze in pagina, il {@code FindListener}
     * di questa piattaforma annuncia un totale che va da 0 a 1 a seconda di quale tasto si
     * preme, e una posizione ferma a 2 in entrambe le direzioni: un contatore preso da lì
     * scriverebbe «0/0» o «3/1» sopra una pagina con due parole evidenziate, cioè una
     * bugia. Il numero lo conta quindi la pagina stessa, sul proprio testo visibile.
     *
     * <p>Ne esce un totale e non una posizione («2», non «1/2»): quale occorrenza si stia
     * guardando è una cosa che il salto della WebView non annuncia in modo affidabile, e
     * un «quale» inventato sarebbe peggio di un totale vero.
     */
    private void cerca(String testo) {
        Scheda t = schedaCorrente();
        if (t == null || t.web == null) return;
        if (testo == null || testo.isEmpty()) { t.web.clearMatches(); findInfo.setText(""); return; }
        t.web.findAllAsync(testo);
        contaOccorrenze(t.web, testo);
    }

    /** Fa contare alla pagina le occorrenze del testo nel suo testo visibile. */
    private void contaOccorrenze(WebView w, String testo) {
        String js = "(function(){try{"
                + "var q=" + JSONObject.quote(testo) + ";"
                + "var t=document.body?document.body.innerText:'';"
                + "var re=new RegExp(q.replace(/[.*+?^${}()|[\\]\\\\]/g,'\\\\$&'),'gi');"
                + "var m=t.match(re);return m?m.length:0;"
                + "}catch(e){return -1;}})()";
        w.evaluateJavascript(js, r -> {
            if (r == null) { findInfo.setText(""); return; }
            r = r.trim();
            if (r.length() > 1 && r.charAt(0) == '"') r = r.substring(1, r.length() - 1);
            findInfo.setText("-1".equals(r) ? "" : r);
        });
    }

    /**
     * Passa all'occorrenza successiva o precedente.
     *
     * <p>Sta separata da {@link #cerca} per una ragione precisa: chiamando {@code findNext}
     * subito dopo {@code findAllAsync} — cioè a ogni lettera digitata — la WebView annuncia
     * una posizione che non ha niente a che vedere con la ricerca appena fatta, e il
     * contatore arriva a dire cose come «3/1»: tre di una. La posizione la sa solo la
     * ricerca in corso, e il salto lo decide il dito.
     */
    private void salta(boolean avanti) {
        Scheda t = schedaCorrente();
        if (t == null || t.web == null) return;
        if (findInput.getText().toString().isEmpty()) return;
        t.web.findNext(avanti);
    }

    private void mostraBarraTrova(boolean mostra) {
        findBar.setVisibility(mostra ? View.VISIBLE : View.GONE);
        if (mostra) { findInput.requestFocus(); }
        else {
            Scheda t = schedaCorrente();
            if (t != null && t.web != null) t.web.clearMatches();
            findInput.setText(""); findInfo.setText(""); hideKeyboard();
        }
    }

    // ---------------------------------------------------------------- traduzione
    //
    // La pagina si legge in un'altra lingua, con la barra che compare sotto quella superiore
    // e la lingua che si sceglie dal menu.
    //
    // Due decisioni si pagano, e vale la pena scriverle.
    //
    // 1. Il testo lo manda fuori il browser, non la pagina. La traduzione si chiede a un
    //    servizio esterno; se la richiesta la facesse la pagina con una «fetch», il sito
    //    tradotto vedrebbe a chi si manda il proprio testo, e una pagina ostile potrebbe
    //    usare quel canale per i fatti suoi. La richiesta parte da qui, con il solo testo
    //    che si sta traducendo, e la pagina riceve il risultato e nient'altro.
    //
    // 2. La lingua della pagina si legge da quello che la pagina dichiara (l'attributo
    //    «lang» di <html>), non indovinandola dal testo. Indovinarla vorrebbe dire mandare
    //    il testo al servizio prima che l'utente abbia chiesto niente: la barra comparirebbe
    //    dopo aver già spedito la pagina. Così invece la barra si limita a offrire, e il
    //    testo esce solo al tocco. Il prezzo è che le pagine che non dichiarano la lingua
    //    non fanno comparire la barra: si passa dal menu, «Traduci…».

    /** Il traduttore. È l'indirizzo pubblico del servizio gratuito: nessuna chiave, nessun
     *  account — e nessuna garanzia, quindi ogni risposta si controlla prima di usarla. */
    private static final String TRADUCI_URL = "https://translate.googleapis.com/translate_a/single";

    /** Quanto testo si manda in una richiesta. Più blocchi si mandano insieme, meno
     *  richieste si fanno; troppo lunghi, e una riga persa dal servizio fa saltare tutto il
     *  blocco (v. applica). Il numero è un compromesso fra i due, e sta alto perché il
     *  servizio è gratuito e non gradisce le raffiche: meglio poche richieste grandi. */
    private static final int TRAD_BLOCCO = 2500;

    /** Quante richieste insieme. Due, non dodici: il servizio non ha una chiave né un
     *  contratto, e risponde a chi lo tempesta con un blocco che dura minuti. */
    private static final int TRAD_FILI = 2;

    /** La pausa fra una richiesta e la successiva dello stesso filo. Fa perdere qualche
     *  secondo su una pagina lunga e evita di farsi riconoscere come una raffica. */
    private static final long TRAD_PAUSA = 400;

    // Gli stati della barra.
    private static final int T_OFFERTA = 0;   // la pagina è in un'altra lingua: si propone
    private static final int T_LAVORO = 1;    // richieste in corso
    private static final int T_TRADOTTA = 2;  // la pagina a video è tradotta
    private static final int T_GUASTO = 3;    // non è riuscita

    private LinearLayout creaTradBar() {
        LinearLayout f = new LinearLayout(this);
        f.setOrientation(LinearLayout.HORIZONTAL);
        f.setGravity(Gravity.CENTER_VERTICAL);
        f.setBackgroundColor(BAR);
        f.setPadding(dp(12), dp(6), dp(8), dp(6));
        f.setVisibility(View.GONE);

        tradIcona = iconaVista(R.drawable.ic_translate, 17, TXT);
        tradIcona.setPadding(dp(2), 0, dp(6), 0);

        tradTesto = new TextView(this);
        tradTesto.setTextColor(TXT);
        tradTesto.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        tradTesto.setLayoutParams(new LinearLayout.LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f));
        // Il testo può essere lungo («Tradotta in italiano · …»): va a capo invece di
        // spingere fuori dalla barra i pulsanti, che sono la sola via d'uscita.
        tradTesto.setMaxLines(2);

        tradAzione = new Button(this);
        tradAzione.setAllCaps(false);
        tradAzione.setTextColor(ACC);
        tradAzione.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        tradAzione.setBackgroundColor(Color.TRANSPARENT);
        tradAzione.setPadding(dp(8), dp(4), dp(8), dp(4));
        tradAzione.setMinWidth(0); tradAzione.setMinimumWidth(0);

        Button opzioni = iconBtn("⋮");
        Button x = iconBtn("✕");
        opzioni.setOnClickListener(v -> foglioTraduzione(schedaCorrente()));
        x.setOnClickListener(v -> mostraBarraTraduzione(false));
        testiTema.add(opzioni); testiTema.add(x);

        f.addView(tradIcona);
        f.addView(tradTesto);
        f.addView(tradAzione);
        f.addView(opzioni);
        f.addView(x);
        return f;
    }

    /** Mostra o nasconde la barra della traduzione. */
    private void mostraBarraTraduzione(boolean mostra) {
        if (tradBar == null) return;
        tradBar.setVisibility(mostra ? View.VISIBLE : View.GONE);
        if (!mostra) { tradTesto.setText(""); tradAzione.setOnClickListener(null); }
    }

    /**
     * Mette la barra nello stato voluto.
     *
     * <p>Il pulsante è uno solo e cambia mestiere con lo stato — «Traduci» quando si offre,
     * «Mostra originale» quando la pagina è tradotta. Due pulsanti contemporanei sarebbero
     * due cose da leggere per una decisione sola; uno che dice sempre la prossima mossa è
     * quello che serve.
     */
    private void barraTraduzione(final Scheda t, int stato) {
        if (tradBar == null || t == null) return;
        String mia = nomeLingua(linguaPreferita());
        switch (stato) {
            case T_LAVORO:
                tradTesto.setText("Traduzione in corso…");
                tradAzione.setText("");
                tradAzione.setOnClickListener(null);
                tradAzione.setEnabled(false);
                break;
            case T_TRADOTTA:
                // La lingua la dice la scheda, non la preferenza: se si cambia la lingua
                // scelta senza ritradurre, la barra deve continuare a dire quella che si sta
                // leggendo adesso, non quella che si è scelta nel frattempo.
                String fatta = nomeLingua(t.linguaMostrata);
                tradTesto.setText(fatta.isEmpty() ? "Pagina tradotta" : "Tradotta in " + fatta);
                tradAzione.setText("Mostra originale");
                tradAzione.setEnabled(true);
                tradAzione.setOnClickListener(v -> mostraOriginale(t));
                break;
            case T_GUASTO:
                tradTesto.setText(tradMessaggio.isEmpty()
                        ? "Traduzione non riuscita: il servizio non ha risposto"
                        : tradMessaggio);
                tradAzione.setText("Riprova");
                tradAzione.setEnabled(true);
                tradAzione.setOnClickListener(v -> traduci(t, linguaPreferita()));
                break;
            default:
                String sua = nomeLingua(t.linguaPagina);
                String testo = sua.isEmpty() ? "Questa pagina è in un'altra lingua" : "Questa pagina è in " + sua;
                tradTesto.setText(mia.isEmpty() ? testo + ". Tradurla?" : testo + ". Tradurla in " + mia + "?");
                tradAzione.setText("Traduci");
                tradAzione.setEnabled(true);
                tradAzione.setOnClickListener(v -> traduci(t, linguaPreferita()));
                break;
        }
        tradBar.setVisibility(View.VISIBLE);
        // Il colore dell'icona segue la barra: in incognito la barra è scura e questa è
        // l'unica cosa che non lo seguiva da sé.
        if (tradIcona != null) tradIcona.setColorFilter(t.incognito ? INC_TXT : TXT);
    }

    /** La lingua dell'interfaccia: è quella del sistema, e l'italiano resta il ripiego. */
    private String linguaInterfaccia() {
        String l = Locale.getDefault().getLanguage();
        if (l == null || l.isEmpty()) return "it";
        for (String[] x : LINGUE) {
            if (x[0].equalsIgnoreCase(l)) return x[0];
        }
        return "it";
    }

    /** La lingua in cui si traduce: quella scelta, o quella dell'interfaccia. */
    private String linguaPreferita() {
        String l = prefs().getString(PREF_LINGUA, "");
        return l == null || l.isEmpty() ? linguaInterfaccia() : l;
    }

    private void impostaLingua(String codice) {
        prefs().edit().putString(PREF_LINGUA, codice).apply();
    }

    /** I siti per cui non si deve tradurre: un elenco di indirizzi. */
    private List<String> sitiNoTraduzione() {
        List<String> out = new ArrayList<>();
        try {
            JSONArray a = new JSONArray(prefs().getString(PREF_NO_TRAD, "[]"));
            for (int i = 0; i < a.length(); i++) out.add(a.optString(i, ""));
        } catch (Exception e) { /* elenco illeggibile: vale come vuoto */ }
        return out;
    }

    private boolean noTraduzione(String host) {
        return host != null && !host.isEmpty() && sitiNoTraduzione().contains(host);
    }

    private void aggiungiNoTraduzione(String host) {
        if (host == null || host.isEmpty()) return;
        List<String> l = sitiNoTraduzione();
        if (!l.contains(host)) l.add(host);
        JSONArray a = new JSONArray();
        for (String s : l) a.put(s);
        prefs().edit().putString(PREF_NO_TRAD, a.toString()).apply();
        // Chi dice «non tradurre mai questo sito» lo dice anche della pagina che ha davanti:
        // togliere la barra e lasciare il testo tradotto sarebbe mezzo servizio.
        Scheda t = schedaCorrente();
        if (t != null && !t.linguaMostrata.isEmpty()) mostraOriginale(t);
        else mostraBarraTraduzione(false);
        Toast.makeText(this, "Non tradurrò più " + host, Toast.LENGTH_SHORT).show();
    }

    /**
     * Chiede alla pagina che lingua dichiara, e da lì decide se offrire la traduzione.
     *
     * <p>Si legge l'attributo e basta: niente testo, niente domande a nessuno. Una pagina che
     * non dichiara la lingua resta senza barra, e chi la vuole tradurre lo chiede dal menu.
     */
    private void leggiLinguaPagina(final Scheda t) {
        if (t == null || t.web == null) return;
        t.web.evaluateJavascript("(function(){try{return document.documentElement.getAttribute('lang')||'';}catch(e){return '';}})()", r -> {
            String l = testoDaJs(r == null ? "" : r);
            t.linguaPagina = l == null ? "" : l.trim();
            if (t != schedaCorrente()) return;
            if (!t.linguaMostrata.isEmpty()) { barraTraduzione(t, T_TRADOTTA); return; }
            String mia = linguaInterfaccia();
            String sua = t.linguaPagina;
            String base = sua.contains("-") ? sua.split("-")[0] : sua;
            if (base.isEmpty() || base.equalsIgnoreCase(mia) || noTraduzione(hostDi(t.indirizzo))) {
                mostraBarraTraduzione(false);
                return;
            }
            barraTraduzione(t, T_OFFERTA);
        });
    }

    /**
     * Traduce la pagina nella lingua indicata.
     *
     * <p>Tre passi: la pagina consegna i suoi testi, il servizio li traduce uno o più blocchi
     * per volta, la pagina li rimette a posto. Il primo e il terzo sono della pagina (solo lei
     * sa quali nodi contengono testo visibile), il secondo è nostro — ed è la ragione per cui
     * il testo non passa per il sito.
     */
    private void traduci(final Scheda t, final String verso) {
        if (t == null || t.web == null) return;
        tradMessaggio = "";
        t.linguaMostrata = "";
        barraTraduzione(t, T_LAVORO);
        // Se una traduzione c'è già (si sta cambiando lingua) si rimette prima l'originale:
        // tradurre un testo già tradotto lo allontana dal suo senso a ogni passaggio.
        t.web.evaluateJavascript(JS_TRADUCI + ";(function(){try{var t=window.__novaTraduzione;if(!t)return null;t.ripristina();return t.raccogli();}catch(e){return null;}})()", r -> {
            JSONArray blocchi = null;
            try {
                Object o = new JSONTokener(r == null ? "null" : r).nextValue();
                if (o instanceof JSONArray) blocchi = (JSONArray) o;
            } catch (Exception e) { /* la pagina non ha risposto come doveva */ }
            if (blocchi == null || blocchi.length() == 0) {
                tradMessaggio = "In questa pagina non c'è testo da tradurre";
                barraTraduzione(t, T_GUASTO);
                return;
            }
            final List<String> daTradurre = new ArrayList<>();
            for (int i = 0; i < blocchi.length(); i++) daTradurre.add(blocchi.optString(i, ""));
            // La stessa traduzione, già fatta per questa pagina: si rimette invece di
            // richiederla. È il caso di chi torna all'originale e poi vuole di nuovo la
            // traduzione — e ogni richiesta risparmiata è una raffica in meno.
            if (t.blocchiTradotti != null && verso.equals(t.linguaBlocchi)
                    && t.blocchiTradotti.size() == daTradurre.size()) {
                applica(t, t.blocchiTradotti, verso, "");
                return;
            }
            traduciBlocchi(t, daTradurre, verso);
        });
    }

    /**
     * Manda i blocchi al servizio e, quando sono tornati, li mette nella pagina.
     *
     * <p>Le richieste vanno in parallelo, ma poche e con una pausa fra l'una e l'altra: il
     * servizio è gratuito, non ha una chiave né un contratto, e a chi lo tempesta risponde
     * con un blocco che dura minuti. Due fili con una pausa breve traducono una pagina lunga
     * in una ventina di secondi, che è il tempo che serve; sei fili senza pause l'avrebbero
     * tradotta in cinque, una volta, e poi bloccata per un quarto d'ora.
     *
     * <p>L'ordine è quello dei blocchi, non quello delle risposte: il posto di ogni pezzo lo
     * decide il suo indice, e chi arriva prima aspetta gli altri nella sua casella. Un blocco
     * che non torna resta nullo, e il suo pezzo di pagina resta in originale — meglio una
     * frase non tradotta che una frase nel posto sbagliato.
     */
    private void traduciBlocchi(final Scheda t, final List<String> daTradurre, final String verso) {
        final int quanti = daTradurre.size();
        final String[] tradotti = new String[quanti];
        final AtomicInteger prossimo = new AtomicInteger(0);
        final AtomicInteger fatti = new AtomicInteger(0);
        final AtomicInteger riusciti = new AtomicInteger(0);
        final AtomicBoolean bloccato = new AtomicBoolean(false);
        final String[] rilevata = { "" };

        Thread[] fili = new Thread[Math.min(TRAD_FILI, quanti)];
        for (int i = 0; i < fili.length; i++) {
            fili[i] = new Thread(() -> {
                int k;
                while (!bloccato.get() && (k = prossimo.getAndIncrement()) < quanti) {
                    Risposta ris = chiediTraduzione(daTradurre.get(k), verso);
                    if (ris != null && ris.bloccato) { bloccato.set(true); break; }
                    if (ris != null && ris.ok) {
                        tradotti[k] = ris.testo;
                        synchronized (rilevata) { if (rilevata[0].isEmpty()) rilevata[0] = ris.lingua; }
                        riusciti.incrementAndGet();
                    }
                    final int avanti = fatti.incrementAndGet();
                    runOnUiThread(() -> {
                        if (t != schedaCorrente() || tradBar == null || tradBar.getVisibility() != View.VISIBLE) return;
                        tradTesto.setText(quanti > 1
                                ? "Traduzione in corso… " + avanti + "/" + quanti
                                : "Traduzione in corso…");
                    });
                    try { Thread.sleep(TRAD_PAUSA); } catch (InterruptedException e) { Thread.currentThread().interrupt(); break; }
                }
            }, "traduzione-" + i);
            fili[i].start();
        }

        new Thread(() -> {
            for (Thread f : fili) { try { f.join(); } catch (InterruptedException e) { Thread.currentThread().interrupt(); } }
            final boolean interrotto = bloccato.get();
            final List<String> risultati = new ArrayList<>();
            for (int i = 0; i < quanti; i++) risultati.add(tradotti[i]);
            runOnUiThread(() -> {
                if (isFinishing() || t.web == null) return;
                if (riusciti.get() == 0) {
                    tradMessaggio = interrotto
                            ? "Il servizio gratuito ha bloccato le richieste: riprova fra qualche minuto"
                            : "Traduzione non riuscita: il servizio non ha risposto";
                    barraTraduzione(t, T_GUASTO);
                    return;
                }
                // Se il blocco è arrivato a metà, la pagina si traduce a pezzi: la parte
                // tradotta si mette, e si dice che è parziale invece di far credere che sia
                // tutta — le frasi rimaste in originale si vedono, ma il perché no.
                if (interrotto) Toast.makeText(this, "Traduzione parziale: il servizio ha bloccato il resto delle richieste", Toast.LENGTH_LONG).show();
                applica(t, risultati, verso, rilevata[0]);
            });
        }, "traduzione-attesa").start();
    }

    /** Mette i blocchi tradotti nella pagina e aggiorna la barra. */
    private void applica(final Scheda t, final List<String> tradotti, final String verso, final String linguaRilevata) {
        if (t.web == null) return;
        JSONArray arr = new JSONArray();
        for (String s : tradotti) arr.put(s == null ? JSONObject.NULL : s);
        t.web.evaluateJavascript(JS_TRADUCI + ";(function(){try{return window.__novaTraduzione.applica(" + arr.toString() + ");}catch(e){return null;}})()", esito -> {
            if (t != schedaCorrente()) return;
            // La pagina dice quanti pezzi ha accettato: se non ne ha accettato nessuno, la
            // traduzione non c'è, e dire «Tradotta in…» sarebbe una bugia con la pagina in
            // originale sotto gli occhi. Succede quando i nodi di testo cambiano fra la
            // lettura e la scrittura — cioè su una pagina che si riscrive da sola.
            int posati = 0;
            boolean rispostaValida = false;
            try {
                Object o = new JSONTokener(esito == null ? "null" : esito).nextValue();
                if (o instanceof JSONObject) { rispostaValida = true; posati = ((JSONObject) o).optInt("fatti", 0); }
            } catch (Exception e) { /* risposta illeggibile: vale come nessun pezzo posato */ }
            if (posati == 0) {
                tradMessaggio = rispostaValida
                        ? "La pagina è cambiata mentre la si traduceva: ricaricala e riprova"
                        : "Traduzione non riuscita: i testi non sono arrivati alla pagina";
                barraTraduzione(t, T_GUASTO);
                return;
            }
            // Si tiene la traduzione solo se la pagina è rimasta la stessa: se nel frattempo
            // si è navigato, questi blocchi sono di un'altra pagina e non servono più.
            t.blocchiTradotti = new ArrayList<>(tradotti);
            t.linguaBlocchi = verso;
            t.linguaMostrata = verso;
            // La lingua che la pagina dichiara la sappiamo ora per certo: se non la
            // dichiarava, il servizio l'ha riconosciuta, e la barra può dirlo.
            if (t.linguaPagina.isEmpty() && linguaRilevata != null && !linguaRilevata.isEmpty()) t.linguaPagina = linguaRilevata;
            barraTraduzione(t, T_TRADOTTA);
        });
    }

    /**
     * La voce «Traduci…» del menu.
     *
     * <p>Se la pagina dichiara una lingua diversa da quella in cui si traduce, si va dritti:
     * chi tocca «Traduci» ha già detto cosa vuole, e fermarsi a chiedere la lingua sarebbe una
     * domanda in più per una risposta che c'è già (è quello che fa il browser di sempre). Se
     * invece la lingua non è dichiarata, o è la stessa, si apre l'elenco: lì la scelta è
     * l'unica cosa che si può chiedere, perché non c'è niente da indovinare.
     */
    private void apriTraduzione(final Scheda t) {
        if (t == null || t.web == null) return;
        if (t.linguaPagina.isEmpty()) { foglioLingue(); return; }
        String sua = t.linguaPagina.contains("-") ? t.linguaPagina.split("-")[0] : t.linguaPagina;
        if (sua.equalsIgnoreCase(linguaPreferita()) || sua.equalsIgnoreCase(linguaInterfaccia())) {
            foglioLingue();
            return;
        }
        traduci(t, linguaPreferita());
    }

    /** Rimette la pagina come l'ha scritta il sito. */
    private void mostraOriginale(final Scheda t) {
        if (t == null || t.web == null) return;
        t.web.evaluateJavascript(JS_TRADUCI + ";(function(){try{return window.__novaTraduzione.ripristina();}catch(e){return null;}})()", r -> {
            t.linguaMostrata = "";
            if (t != schedaCorrente()) return;
            // Un sito escluso non deve ritrovarsi la barra «Traduci» sotto gli occhi: chi
            // l'ha escluso ha già detto che lì non si traduce.
            if (noTraduzione(hostDi(t.indirizzo))) mostraBarraTraduzione(false);
            else barraTraduzione(t, T_OFFERTA);
        });
    }

    /** Quello che torna dal servizio: il testo tradotto e la lingua che ha riconosciuto. */
    private static class Risposta {
        boolean ok;
        /** Il servizio ci ha risposto di no, e non per un guasto: ci ha bloccati. */
        boolean bloccato;
        String testo = "";
        String lingua = "";
    }

    /**
     * Chiede la traduzione di un blocco al servizio.
     *
     * <p>La lingua di partenza si lascia decidere a lui («auto»): la pagina può dichiarare
     * una lingua e contenerne un'altra, e chi traduce lo vede meglio di chi legge un
     * attributo. La risposta si controlla prima di usarla — lunghezza, forma, e che il
     * numero di righe sia quello che gli abbiamo dato: il servizio è gratuito e non
     * promette niente, quindi non ci si fida.
     *
     * <p>Il blocco si riconosce e si distingue da un guasto, perché le due cose vogliono
     * due risposte diverse: un guasto si riprova subito, un blocco si aspetta. Chi ci ha
     * bloccati non risponde con un errore ma con un rimando a una pagina di scuse: gli si
     * dice di non seguire i rimandi, e quel rimando diventa la notizia.
     */
    private Risposta chiediTraduzione(String testo, String verso) {
        HttpURLConnection c = null;
        try {
            String corpo = "client=gtx&sl=auto&tl=" + URLEncoder.encode(verso, "UTF-8")
                    + "&dt=t&q=" + URLEncoder.encode(testo, "UTF-8");
            c = (HttpURLConnection) apri(new URL(TRADUCI_URL));
            c.setRequestMethod("POST");
            c.setDoOutput(true);
            c.setInstanceFollowRedirects(false);
            c.setConnectTimeout(8000);
            c.setReadTimeout(20000);
            c.setRequestProperty("Content-Type", "application/x-www-form-urlencoded; charset=utf-8");
            c.setRequestProperty("User-Agent", UA_DESKTOP);
            OutputStream out = c.getOutputStream();
            out.write(corpo.getBytes("UTF-8"));
            out.flush(); out.close();

            int codice = c.getResponseCode();
            if (codice == 429 || (codice >= 300 && codice < 400)) {
                Risposta ris = new Risposta();
                ris.bloccato = true;
                return ris;
            }
            if (codice != 200) return null;

            StringBuilder sb = new StringBuilder();
            try (BufferedReader r = new BufferedReader(new InputStreamReader(c.getInputStream(), "UTF-8"))) {
                String riga;
                while ((riga = r.readLine()) != null) sb.append(riga);
            }
            JSONArray radice = new JSONArray(sb.toString());
            JSONArray segmenti = radice.getJSONArray(0);
            StringBuilder t = new StringBuilder();
            for (int i = 0; i < segmenti.length(); i++) {
                JSONArray s = segmenti.optJSONArray(i);
                if (s != null && s.length() > 0 && !s.isNull(0)) t.append(s.optString(0, ""));
            }
            Risposta ris = new Risposta();
            ris.testo = t.toString();
            ris.ok = !ris.testo.isEmpty();
            if (radice.length() > 2 && !radice.isNull(2)) ris.lingua = radice.optString(2, "");
            return ris;
        } catch (Exception e) {
            Log.w(TAG, "traduzione non riuscita", e);
            return null;
        } finally {
            if (c != null) c.disconnect();
        }
    }

    /**
     * Il codice che gira dentro la pagina: prende i testi, li consegna, li rimette a posto.
     *
     * <p>Sta tutto qui dentro e non nel file della pagina: la pagina non deve poterlo
     * intercettare né modificare, e l'unica cosa che riceve è il testo tradotto.
     *
     * <p>Si saltano gli elementi in cui il testo non è prosa — script, stili, moduli, codice —
     * e i nodi troppo corti: tradurre «·», «1» o «» costa una richiesta e non serve a niente.
     * I testi di uno stesso blocco si uniscono con un ritorno a capo, e il numero di righe
     * della risposta deve tornare: se non torna, quel blocco resta in originale invece di
     * spostare le frasi da un nodo all'altro (v. «applica»).
     *
     * <p>La definizione è **idempotente** (`window.__novaTraduzione = window.__novaTraduzione
     * || …`) e questo non è un dettaglio: la traduzione si fa in tre iniezioni distinte —
     * raccogliere, applicare, ripristinare — e se ciascuna ridefinisse il traduttore
     * azzererebbe l'elenco dei nodi raccolti, lasciando «applica» senza niente da riempire.
     * Il primo giro del codice girava così, e sul dispositivo la traduzione finiva con zero
     * nodi riempiti su una pagina intera.
     */
    private static final String JS_TRADUCI =
        "window.__novaTraduzione=window.__novaTraduzione||(function(){"
      + "var nodi=[],orig=[],blocchi=[];"
      + "function salta(t){return /^(SCRIPT|STYLE|NOSCRIPT|TEXTAREA|IFRAME|CODE|PRE|SVG|CANVAS|SELECT|OPTION|MATH)$/.test(t);}"
      + "function raccogli(){"
      + "  nodi=[];orig=[];blocchi=[];"
      + "  if(!document.body)return [];"
      + "  var w=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT,{acceptNode:function(n){"
      + "    var v=n.nodeValue; if(!v||!v.trim())return NodeFilter.FILTER_REJECT;"
      + "    var p=n.parentNode;"
      + "    while(p&&p!==document.body){ if(salta(p.nodeName))return NodeFilter.FILTER_REJECT; p=p.parentNode; }"
      + "    if(v.trim().length<2)return NodeFilter.FILTER_REJECT;"
      + "    return NodeFilter.FILTER_ACCEPT; }});"
      + "  var n; while((n=w.nextNode())){ nodi.push(n); orig.push(n.nodeValue); }"
      + "  var cur=[],lung=0;"
      + "  for(var i=0;i<nodi.length;i++){"
      + "    var s=nodi[i].nodeValue.replace(/\\s+/g,' ').trim();"
      + "    if(lung+s.length>" + TRAD_BLOCCO + "&&cur.length){ blocchi.push(cur); cur=[]; lung=0; }"
      + "    cur.push(s); lung+=s.length+1; }"
      + "  if(cur.length)blocchi.push(cur);"
      + "  var out=[]; for(var i=0;i<blocchi.length;i++)out.push(blocchi[i].join('\\n'));"
      + "  return out; }"
      + "function applica(tradotti){"
      + "  var k=0,fatti=0,saltati=0;"
      + "  for(var i=0;i<blocchi.length;i++){"
      + "    var grezzo=(tradotti&&tradotti[i]!=null)?String(tradotti[i]):'';"
      + "    var righe=grezzo.split('\\n');"
      + "    if(righe.length!==blocchi[i].length){ k+=blocchi[i].length; saltati+=blocchi[i].length; continue; }"
      + "    for(var j=0;j<blocchi[i].length;j++){"
      + "      try{"
      + "        var o=orig[k]||'';"
      + "        var pre=o.match(/^\\s*/)[0], post=o.match(/\\s*$/)[0];"
      + "        nodi[k].nodeValue=pre+righe[j].replace(/^\\s+|\\s+$/g,'')+post; fatti++;"
      + "      }catch(e){}"
      + "      k++; } }"
      + "  try{ document.documentElement.setAttribute('data-nova-tradotta','1'); }catch(e){}"
      + "  return {'fatti':fatti,'saltati':saltati}; }"
      + "function ripristina(){"
      + "  for(var i=0;i<nodi.length;i++){ try{ nodi[i].nodeValue=orig[i]; }catch(e){} }"
      + "  nodi=[];orig=[];blocchi=[];"
      + "  try{ document.documentElement.removeAttribute('data-nova-tradotta'); }catch(e){}"
      + "  return true; }"
      + "return {raccogli:raccogli,applica:applica,ripristina:ripristina};"
      + "})();";

    /**
     * Il foglio per scegliere la lingua.
     *
     * <p>Le lingue si cercano: l'elenco è lungo e chi sa già cosa cerca non deve scorrerlo.
     * La ricerca guarda il nome italiano e il codice, perché «pt» si cerca così come
     * «portoghese»; la lingua in uso ha la spunta.
     */
    private void foglioLingue() {
        final String attuale = linguaPreferita();
        LinearLayout corpo = apriFoglio("Traduci in…", "Annulla");

        EditText cerca = new EditText(this);
        cerca.setSingleLine(true);
        cerca.setHint("Cerca una lingua");
        cerca.setHintTextColor(colDim());
        cerca.setTextColor(colTxt());
        cerca.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        GradientDrawable sfondo = new GradientDrawable();
        sfondo.setColor(colCap());
        sfondo.setCornerRadius(dp(20));
        cerca.setBackground(sfondo);
        cerca.setPadding(dp(16), dp(10), dp(16), dp(10));
        LinearLayout.LayoutParams clp = new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT);
        clp.setMargins(dp(16), dp(4), dp(16), dp(10));
        cerca.setLayoutParams(clp);
        corpo.addView(cerca);

        final LinearLayout lista = new LinearLayout(this);
        lista.setOrientation(LinearLayout.VERTICAL);
        corpo.addView(lista);

        cerca.addTextChangedListener(new TextWatcher() {
            public void beforeTextChanged(CharSequence s, int a, int b, int c) {}
            public void afterTextChanged(Editable s) {}
            public void onTextChanged(CharSequence s, int a, int b, int c) { riempiLingue(lista, s.toString(), attuale); }
        });
        riempiLingue(lista, "", attuale);
    }

    private void riempiLingue(LinearLayout lista, String filtro, final String attuale) {
        lista.removeAllViews();
        String f = filtro == null ? "" : filtro.trim().toLowerCase(Locale.ITALY);
        for (final String[] l : LINGUE) {
            if (!f.isEmpty() && !l[1].toLowerCase(Locale.ITALY).contains(f) && !l[0].toLowerCase(Locale.ITALY).startsWith(f)) continue;
            lista.addView(rigaScelta(l[1], l[0].equalsIgnoreCase(attuale), () -> {
                impostaLingua(l[0]);
                Scheda t = schedaCorrente();
                if (t != null) traduci(t, l[0]);
            }));
        }
        if (lista.getChildCount() == 0) {
            TextView n = new TextView(this);
            n.setText("Nessuna lingua con questo nome.");
            n.setTextColor(colDim());
            n.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
            n.setPadding(dp(22), dp(16), dp(22), dp(16));
            lista.addView(n);
        }
    }

    /** Il ⋮ della barra: la lingua, il silenzio per questo sito, e nient'altro. */
    private void foglioTraduzione(final Scheda t) {
        if (t == null) return;
        final String host = hostDi(t.indirizzo);
        final String fatta = nomeLingua(t.linguaMostrata);
        // Il titolo dice quello che c'è a video, non quello che si vorrebbe: aprendo questo
        // foglio dalla barra di un guasto, «Tradotto in italiano» sarebbe una bugia scritta
        // in cima a una pagina che è ancora in inglese.
        LinearLayout corpo = apriFoglio(fatta.isEmpty() ? "Traduzione" : "Tradotto in " + fatta, "Chiudi");
        corpo.addView(rigaFoglio("Scegli la lingua", nomeLingua(linguaPreferita()), this::foglioLingue));
        if (!t.linguaMostrata.isEmpty()) {
            corpo.addView(rigaFoglio("Mostra originale", nomeLingua(t.linguaPagina).isEmpty() ? "la pagina come l'ha scritta il sito" : "in " + nomeLingua(t.linguaPagina), () -> mostraOriginale(t)));
        } else if (!t.linguaPagina.isEmpty()) {
            corpo.addView(rigaFoglio("Traduci questa pagina", nomeLingua(t.linguaPagina), () -> traduci(t, linguaPreferita())));
        }
        if (!host.isEmpty()) {
            corpo.addView(rigaFoglio("Non tradurre mai questo sito", host, () -> aggiungiNoTraduzione(host)));
        }
        // La cosa da dire, e va detta qui dove si decide: tradurre vuol dire mandare il testo
        // della pagina a un servizio esterno. In incognito vale lo stesso, e va detto doppio.
        TextView nota = new TextView(this);
        nota.setText("Per tradurre, il testo della pagina viene inviato al servizio di traduzione."
                + (t.incognito ? "\n\nQuesta scheda è in incognito: qui non si salva niente sul telefono, ma il testo esce lo stesso dal dispositivo." : ""));
        nota.setTextColor(colDim());
        nota.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        nota.setPadding(dp(22), dp(12), dp(22), dp(6));
        corpo.addView(nota);
    }

    // ---------------------------------------------------------- preferiti / cronologia

    // Un elenco solo, condiviso con la shell.
    //
    // Il browser nativo e l'app Browser dentro la shell avevano due elenchi indipendenti:
    // un preferito salvato in uno non si vedeva nell'altro. Qui si usa la stessa copia che
    // la shell aggiorna a ogni modifica — le preferenze native «novaos», chiavi
    // nova:bookmarks e nova:browserHistory — e le stesse forme delle voci che scrive la
    // shell: {name, url} per i preferiti, {url, t} per la cronologia (dalla più recente).
    // Così un preferito salvato nella shell si vede qui senza alcun lavoro in più.
    //
    // Il verso opposto — uno salvato qui che compare nella shell — ha bisogno che la shell
    // riceva la notizia, perché legge la propria localStorage: se ne occupa
    // MainActivity.preferitiCambiati(), chiamata a ogni modifica di questo elenco.

    private SharedPreferences prefs() { return getSharedPreferences("nyctalus_browser", MODE_PRIVATE); }

    private JSONArray arr(String k) {
        try { return new JSONArray(prefs().getString("nova:" + k, "[]")); } catch (Exception e) { return new JSONArray(); }
    }
    private void putArr(String k, JSONArray a) { prefs().edit().putString("nova:" + k, a.toString()).apply(); }

    /** Il nome di un preferito: la shell scrive «name», le versioni precedenti «title». */
    private static String nomePreferito(JSONObject o) {
        String n = o.optString("name", "");
        if (n.isEmpty()) n = o.optString("title", "");
        if (n.isEmpty()) n = o.optString("url");
        return n;
    }

    private void addPreferito(String titolo, String url) {
        if (url == null || url.isEmpty()) return;
        try {
            JSONArray a = arr("bookmarks");
            for (int i = 0; i < a.length(); i++) if (url.equals(a.getJSONObject(i).optString("url"))) {
                Toast.makeText(this, "Già nei preferiti", Toast.LENGTH_SHORT).show(); return;
            }
            JSONObject o = new JSONObject();
            o.put("name", titolo == null || titolo.isEmpty() ? url : titolo);
            o.put("url", url);
            a.put(o); putArr("bookmarks", a);
            Toast.makeText(this, "Aggiunto ai preferiti", Toast.LENGTH_SHORT).show();
            preferitiCambiati();
        } catch (Exception e) { Log.w(TAG, "preferito non salvato", e); }
    }

    private void addCronologia(String url, String titolo) {
        if (url == null || url.isEmpty() || url.startsWith("data:")) return;
        try {
            JSONArray a = arr("browserHistory");
            JSONArray out = new JSONArray();
            JSONObject o = new JSONObject();
            o.put("url", url);
            // Il titolo della pagina, che il browser conosce: la shell prima salvava solo
            // l'indirizzo, e l'elenco era una lista di indirizzi. Le voci vecchie non ce
            // l'hanno, e l'elenco le mostra col sito — v. nomeCronologia.
            if (titolo != null && !titolo.isEmpty()) o.put("name", titolo);
            o.put("t", System.currentTimeMillis());
            out.put(o);
            // 30 voci, come la shell: è la sua stessa coda, e allungarla qui vorrebbe dire
            // che il primo caricamento della shell la accorcia di nuovo.
            for (int i = 0; i < a.length() && out.length() < 30; i++) {
                JSONObject e = a.getJSONObject(i);
                if (!url.equals(e.optString("url"))) out.put(e);
            }
            putArr("browserHistory", out);
        } catch (Exception e) { Log.w(TAG, "cronologia non aggiornata", e); }
    }

    private boolean eNeiPreferiti(String url) {
        if (url == null || url.isEmpty()) return false;
        JSONArray a = arr("bookmarks");
        for (int i = 0; i < a.length(); i++) {
            JSONObject o = a.optJSONObject(i);
            if (o != null && url.equals(o.optString("url"))) return true;
        }
        return false;
    }

    /** Aggiunge o rimuove il preferito (toggle) e mostra un avviso. */
    private void invertiPreferito(String titolo, String url) {
        if (url == null || url.isEmpty()) return;
        if (eNeiPreferiti(url)) { rimuoviPreferito(url); Toast.makeText(this, "Rimosso dai preferiti", Toast.LENGTH_SHORT).show(); }
        else addPreferito(titolo, url);
        // La stella del menu si ridisegna qui, non alla riapertura del menu: il pannello
        // resta aperto dopo il tocco, ed è la stella a dire se il salvataggio è avvenuto.
        aggiornaStellaUI();
    }

    /** Il tocco sulla stella (della fila in alto o della voce): agisce sulla scheda in vista. */
    private void invertiPreferitoDaMenu(View v) {
        Scheda c = schedaCorrente();
        if (c == null || c.indirizzo == null || c.indirizzo.isEmpty()) return;
        invertiPreferito(c.titolo, c.indirizzo);
    }

    /**
     * Ridisegna le stelle dei preferiti secondo lo stato <b>vero</b> della pagina.
     *
     * <p>Blu quando la pagina è salvata — è il segno che il salvataggio è avvenuto — e del
     * colore del testo quando non lo è. Prima la stella della fila restava com'era al
     * momento dell'apertura del menu: il salvataggio si vedeva solo chiudendo e riaprendo.
     * I riferimenti sono nulli quando il menu è chiuso, e in quel caso non c'è nulla da
     * ridisegnare.
     */
    private void aggiornaStellaUI() {
        Scheda s = schedaCorrente();
        boolean bm = s != null && eNeiPreferiti(s.indirizzo);
        int su = bm ? ACC : (s != null && s.incognito ? INC_TXT : TXT);
        if (stellaFila != null) { stellaFila.setText(bm ? "★" : "☆"); stellaFila.setTextColor(su); }
        if (stellaVoceIcona != null) { stellaVoceIcona.setText(bm ? "★" : "☆"); stellaVoceIcona.setTextColor(su); }
        if (stellaVoceTesto != null) stellaVoceTesto.setText(bm ? "Rimuovi dai preferiti" : "Aggiungi ai preferiti");
    }

    private void rimuoviPreferito(String url) {
        try {
            JSONArray a = arr("bookmarks"), out = new JSONArray();
            for (int i = 0; i < a.length(); i++) {
                JSONObject o = a.getJSONObject(i);
                if (!url.equals(o.optString("url"))) out.put(o);
            }
            putArr("bookmarks", out);
            preferitiCambiati();
        } catch (Exception e) { Log.w(TAG, "preferito non rimosso", e); }
    }

    private void rimuoviPreferitoIndice(int idx) {
        try {
            JSONArray a = arr("bookmarks"), out = new JSONArray();
            for (int i = 0; i < a.length(); i++) if (i != idx) out.put(a.get(i));
            putArr("bookmarks", out);
            preferitiCambiati();
        } catch (Exception e) { Log.w(TAG, "preferito non rimosso", e); }
    }

    // =====================================================================
    //  Le superfici del browser: gli elenchi, i fogli e gli avvisi
    //
    //  Tutto quello che il browser apre sopra la pagina — la cronologia, i preferiti, le
    //  impostazioni di un sito, la conferma di una cancellazione — è disegnato qui. Prima
    //  erano finestrelle di sistema: grigie, coi pulsanti in fondo a destra, i caratteri di
    //  un'altra app e il titolo in grassetto blu — cioè un pezzo di Android in mezzo a
    //  NovaOS, ogni volta diverso da tutto il resto. Qui ci sono tre sole forme, e sono le
    //  stesse tre di un browser: l'elenco a tutto schermo (cronologia, preferiti), il foglio
    //  che sale dal basso (le scelte), l'avviso al centro (le conferme).
    // =====================================================================

    private FrameLayout elenco;        // elenco a tutto schermo aperto (null = nessuno)
    private LinearLayout elencoCorpo;  // dove vanno le righe
    private boolean elencoPreferiti;   // quale dei due elenchi è aperto
    private String elencoFiltro = "";  // il testo cercato nel campo in alto

    private FrameLayout foglio;        // foglio sollevato dal basso (null = chiuso)
    private FrameLayout avviso;        // avviso al centro (null = chiuso)

    /** Una scelta nell'avviso: il testo del pulsante, se è quello principale, e che cosa fa. */
    private static class Pulsante {
        final String testo; final boolean forte; final Runnable azione;
        Pulsante(String testo, boolean forte, Runnable azione) { this.testo = testo; this.forte = forte; this.azione = azione; }
        static Pulsante normale(String testo, Runnable azione) { return new Pulsante(testo, false, azione); }
        static Pulsante principale(String testo, Runnable azione) { return new Pulsante(testo, true, azione); }
    }

    /** Vale per la scheda in vista: in incognito le superfici sono scure. */
    private boolean incognitoOra() {
        Scheda s = schedaCorrente();
        return s != null && s.incognito;
    }
    private int colTxt() { return incognitoOra() ? INC_TXT : TXT; }
    private int colDim() { return incognitoOra() ? INC_DIM : DIM; }
    private int colCard() { return incognitoOra() ? INC_CARD : CARD; }
    private int colCap() { return incognitoOra() ? INC_CAP : CAP; }

    // ------------------------------------------------------------ il foglio dal basso
    /**
     * Apre un foglio dal basso con il suo titolo, e restituisce il corpo da riempire.
     *
     * <p>Si usa così: si chiama, si aggiungono le righe con {@link #rigaFoglio}, e il foglio
     * si compone da sé — titolo in alto, righe in mezzo, la via d'uscita in fondo. La riga
     * d'uscita sta fuori dallo scorrimento apposta: se le voci fossero tante da dover
     * scorrere, il modo di chiudere non deve scorrere via con loro.
     *
     * @param titolo  l'intestazione, oppure {@code null} per un foglio di sole voci
     * @param chiudi  il testo del pulsante di chiusura ({@code null} per non metterlo)
     */
    private LinearLayout apriFoglio(String titolo, String chiudi) {
        chiudiFoglio();
        chiudiAvviso();
        final int cCard = colCard(), cTxt = colTxt(), cDim = colDim();

        LinearLayout scheda = new LinearLayout(this);
        scheda.setOrientation(LinearLayout.VERTICAL);
        GradientDrawable sfondo = new GradientDrawable();
        sfondo.setColor(cCard);
        sfondo.setCornerRadii(new float[]{ dp(20), dp(20), dp(20), dp(20), 0, 0, 0, 0 });
        scheda.setBackground(sfondo);

        if (titolo != null && !titolo.isEmpty()) {
            TextView t = new TextView(this);
            t.setText(titolo);
            t.setTextColor(cDim);
            t.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
            t.setPadding(dp(22), dp(20), dp(22), dp(6));
            t.setMaxLines(2);
            scheda.addView(t);
        }

        LinearLayout corpo = new LinearLayout(this);
        corpo.setOrientation(LinearLayout.VERTICAL);
        ScrollView scorri = new ScrollView(this);
        scorri.addView(corpo);
        scheda.addView(scorri, new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT));

        if (chiudi != null) {
            View riga = new View(this);
            riga.setBackgroundColor(cDim);
            riga.setAlpha(0.18f);
            scheda.addView(riga, new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, Math.max(1, dp(1) / 2)));
            TextView x = new TextView(this);
            x.setText(chiudi);
            x.setTextColor(cTxt);
            x.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
            x.setGravity(Gravity.CENTER);
            x.setPadding(dp(22), dp(16), dp(22), dp(18));
            x.setBackgroundResource(sfondoTocco());
            x.setOnClickListener(v -> chiudiFoglio());
            scheda.addView(x);
        }

        foglio = vetro(scheda, Gravity.BOTTOM);
        // Un foglio più alto dello schermo si ferma prima del bordo: senza, le prime voci
        // finirebbero fuori dalla cornice e non si potrebbero toccare.
        scheda.post(() -> {
            int max = (int) (getResources().getDisplayMetrics().heightPixels * 0.78f) - dp(24);
            if (scheda.getHeight() > max) {
                LinearLayout.LayoutParams lp = (LinearLayout.LayoutParams) scorri.getLayoutParams();
                lp.height = max;
                scorri.setLayoutParams(lp);
            }
        });
        return corpo;
    }

    /** Una riga di un foglio: il testo, un dettaglio in coda, e cosa fare al tocco. */
    private View rigaFoglio(String testo, String dettaglio, Runnable azione) {
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.HORIZONTAL);
        r.setGravity(Gravity.CENTER_VERTICAL);
        r.setPadding(dp(22), dp(15), dp(22), dp(15));
        r.setBackgroundResource(sfondoTocco());

        TextView t = new TextView(this);
        t.setText(testo);
        t.setTextColor(colTxt());
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        t.setLayoutParams(new LinearLayout.LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f));
        r.addView(t);

        if (dettaglio != null && !dettaglio.isEmpty()) {
            TextView d = new TextView(this);
            d.setText(dettaglio);
            d.setTextColor(colDim());
            d.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
            d.setPadding(dp(12), 0, 0, 0);
            r.addView(d);
        }
        if (azione != null) r.setOnClickListener(v -> { chiudiFoglio(); azione.run(); });
        return r;
    }

    /** Una riga scelta fra le alternative: la spunta sta in coda a quella in corso. */
    private View rigaScelta(String testo, boolean scelta, Runnable azione) {
        LinearLayout r = (LinearLayout) rigaFoglio(testo, null, azione);
        TextView s = new TextView(this);
        s.setText("✓");
        s.setTextColor(scelta ? ACC : Color.TRANSPARENT);
        s.setTextSize(TypedValue.COMPLEX_UNIT_SP, 16);
        s.setPadding(dp(12), 0, 0, 0);
        r.addView(s);
        return r;
    }

    private void chiudiFoglio() {
        if (foglio != null && foglio.getParent() != null) ((ViewGroup) foglio.getParent()).removeView(foglio);
        foglio = null;
    }

    // ------------------------------------------------------------ l'avviso al centro
    /**
     * Un avviso: titolo, testo, e i pulsanti passati — che si mettono in fila da sinistra a
     * destra nell'ordine in cui li si scrive, con il principale alla fine, cioè a destra,
     * dove sta sempre il pulsante che conclude.
     */
    private void mostraAvviso(String titolo, String messaggio, View extra, Pulsante... pulsanti) {
        chiudiAvviso();
        chiudiFoglio();
        final int cCard = colCard(), cTxt = colTxt(), cDim = colDim();

        LinearLayout scheda = new LinearLayout(this);
        scheda.setOrientation(LinearLayout.VERTICAL);
        GradientDrawable sfondo = new GradientDrawable();
        sfondo.setColor(cCard);
        sfondo.setCornerRadius(dp(18));
        scheda.setBackground(sfondo);
        int pad = dp(22);
        scheda.setPadding(pad, pad, pad, dp(12));

        if (titolo != null && !titolo.isEmpty()) {
            TextView t = new TextView(this);
            t.setText(titolo);
            t.setTextColor(cTxt);
            t.setTextSize(TypedValue.COMPLEX_UNIT_SP, 17);
            t.setPadding(0, 0, 0, dp(8));
            scheda.addView(t);
        }
        if (messaggio != null && !messaggio.isEmpty()) {
            TextView m = new TextView(this);
            m.setText(messaggio);
            m.setTextColor(cDim);
            m.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
            m.setLineSpacing(0, 1.15f);
            scheda.addView(m);
        }
        if (extra != null) { scheda.addView(extra); }

        LinearLayout fila = new LinearLayout(this);
        fila.setOrientation(LinearLayout.HORIZONTAL);
        fila.setGravity(Gravity.END);
        fila.setPadding(0, dp(6), 0, 0);
        for (final Pulsante p : pulsanti) {
            TextView b = new TextView(this);
            b.setText(p.testo);
            b.setTextColor(p.forte ? ACC : cDim);
            b.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
            b.setPadding(dp(14), dp(12), dp(6), dp(10));
            b.setBackgroundResource(sfondoTocco());
            b.setOnClickListener(v -> {
                chiudiAvviso();
                // L'azione dopo la chiusura: aprire un altro avviso da dentro questo no —
                // si sovrapporrebbero, e il secondo resterebbe sotto il primo.
                if (p.azione != null) p.azione.run();
            });
            fila.addView(b);
        }
        scheda.addView(fila, new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT));

        // La larghezza dell'avviso: mai più stretta di 280 dp e mai oltre il bordo dello
        // schermo, con 28 dp di margine per lato — così su un telefono sta al centro e su
        // uno schermo grande non si allarga fino a diventare illeggibile.
        int larg = Math.min(dp(400), getResources().getDisplayMetrics().widthPixels - dp(56));
        FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(larg, LayoutParams.WRAP_CONTENT);
        lp.gravity = Gravity.CENTER;
        avviso = vetro(scheda, Gravity.CENTER, lp);
    }

    private void chiudiAvviso() {
        if (avviso != null && avviso.getParent() != null) ((ViewGroup) avviso.getParent()).removeView(avviso);
        avviso = null;
    }

    /**
     * Il vetro su cui appoggia un foglio o un avviso: il velo scuro che copre la pagina e,
     * dentro, il pannello. Il tocco sul velo chiude, e non arriva alla pagina: sotto non c'è
     * niente da premere.
     */
    private FrameLayout vetro(View pannello, int dove) {
        FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT);
        lp.gravity = dove;
        return vetro(pannello, dove, lp);
    }

    private FrameLayout vetro(View pannello, int dove, FrameLayout.LayoutParams lp) {
        FrameLayout vetro = new FrameLayout(this);
        vetro.setBackgroundColor(0xB3000000);
        vetro.setClickable(true);
        vetro.setOnClickListener(v -> { chiudiFoglio(); chiudiAvviso(); });
        vetro.setPadding(0, dp(24), 0, 0);
        lp.gravity = dove;
        vetro.addView(pannello, lp);
        ((ViewGroup) findViewById(android.R.id.content)).addView(vetro,
                new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        return vetro;
    }

    // ------------------------------------------------------------ l'elenco a tutto schermo
    private void mostraCronologia() { mostraElenco(false); }
    private void mostraPreferiti() { mostraElenco(true); }

    /**
     * L'elenco delle pagine (cronologia o preferiti), a tutto schermo.
     *
     * <p>È una schermata e non una finestrella perché è quello che è: un elenco lungo, da
     * leggere, da cercare e da scorrere. In cima la barra col titolo, il cestino e il campo
     * di ricerca; sotto le righe con il titolo della pagina e il sito, raggruppate per
     * giorno nella cronologia. Il tocco su una riga la apre, il ⋮ accanto (o il dito tenuto
     * premuto) apre le sue azioni.
     */
    private void mostraElenco(boolean preferiti) {
        chiudiElenco();
        chiudiFoglio();
        chiudiAvviso();
        elencoPreferiti = preferiti;
        elencoFiltro = "";

        final int cBar = incognitoOra() ? INC_BAR : BAR, cBg = incognitoOra() ? INC_BG : BG,
                  cTxt = colTxt(), cDim = colDim(), cCap = colCap();

        FrameLayout tutto = new FrameLayout(this);
        tutto.setBackgroundColor(cBg);
        tutto.setClickable(true);

        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);

        // ---- barra: chiudi, titolo, cestino ----
        LinearLayout testata = new LinearLayout(this);
        testata.setOrientation(LinearLayout.VERTICAL);
        testata.setBackgroundColor(cBar);

        LinearLayout top = new LinearLayout(this);
        top.setOrientation(LinearLayout.HORIZONTAL);
        top.setGravity(Gravity.CENTER_VERTICAL);
        top.setPadding(dp(6), dp(10), dp(6), dp(4));
        Button x = iconBtn("✕"); x.setTextColor(cTxt);
        x.setOnClickListener(v -> chiudiElenco());
        TextView tit = new TextView(this);
        tit.setText(preferiti ? "Preferiti" : "Cronologia");
        tit.setTextColor(cTxt);
        tit.setTextSize(TypedValue.COMPLEX_UNIT_SP, 19);
        tit.setPadding(dp(8), 0, 0, 0);
        tit.setLayoutParams(new LinearLayout.LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f));
        top.addView(x); top.addView(tit);
        if (!preferiti) {
            ImageView cestino = iconaBtn(R.drawable.ic_trash);
            cestino.setColorFilter(cTxt, PorterDuff.Mode.SRC_IN);
            cestino.setOnClickListener(v -> eliminaCronologia());
            top.addView(cestino);
        }
        testata.addView(top);

        // ---- il campo di ricerca, con la lente dentro la capsula ----
        LinearLayout capsula = new LinearLayout(this);
        capsula.setOrientation(LinearLayout.HORIZONTAL);
        capsula.setGravity(Gravity.CENTER_VERTICAL);
        GradientDrawable sfondoCap = new GradientDrawable();
        sfondoCap.setColor(cCap);
        sfondoCap.setCornerRadius(dp(22));
        capsula.setBackground(sfondoCap);
        capsula.setPadding(dp(16), dp(1), dp(16), dp(1));
        ImageView lente = iconaVista(R.drawable.ic_find, 16, cDim);
        EditText cerca = new EditText(this);
        cerca.setBackgroundColor(Color.TRANSPARENT);
        cerca.setHint(preferiti ? "Cerca nei preferiti" : "Cerca nella cronologia");
        cerca.setTextColor(cTxt); cerca.setHintTextColor(cDim);
        cerca.setSingleLine(true);
        cerca.setInputType(InputType.TYPE_CLASS_TEXT);
        cerca.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        cerca.setPadding(dp(10), dp(11), 0, dp(11));
        capsula.addView(lente);
        capsula.addView(cerca, new LinearLayout.LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f));
        LinearLayout.LayoutParams clp = new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT);
        clp.setMargins(dp(14), dp(2), dp(14), dp(12));
        capsula.setLayoutParams(clp);
        testata.addView(capsula);
        col.addView(testata);

        elencoCorpo = new LinearLayout(this);
        elencoCorpo.setOrientation(LinearLayout.VERTICAL);
        elencoCorpo.setPadding(0, dp(4), 0, dp(28));
        ScrollView scorri = new ScrollView(this);
        scorri.addView(elencoCorpo);
        col.addView(scorri, new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f));

        tutto.addView(col, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        elenco = tutto;
        ((ViewGroup) findViewById(android.R.id.content)).addView(tutto,
                new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));

        cerca.addTextChangedListener(new TextWatcher() {
            public void beforeTextChanged(CharSequence s, int a, int b, int c) {}
            public void onTextChanged(CharSequence s, int a, int b, int c) { elencoFiltro = s.toString(); riempiElenco(); }
            public void afterTextChanged(Editable s) {}
        });
        riempiElenco();
    }

    private void chiudiElenco() {
        if (elenco != null && elenco.getParent() != null) ((ViewGroup) elenco.getParent()).removeView(elenco);
        elenco = null;
        elencoCorpo = null;
    }

    /** Ridisegna le righe dell'elenco secondo il testo cercato. */
    private void riempiElenco() {
        if (elencoCorpo == null) return;
        elencoCorpo.removeAllViews();
        String cerca = elencoFiltro.trim().toLowerCase(Locale.ITALY);
        JSONArray a = arr(elencoPreferiti ? "bookmarks" : "browserHistory");
        int mostrate = 0;
        String gruppo = null;
        for (int i = 0; i < a.length(); i++) {
            JSONObject o = a.optJSONObject(i);
            if (o == null) continue;
            String url = o.optString("url", "");
            String nome = elencoPreferiti ? nomePreferito(o) : nomeCronologia(o);
            if (!cerca.isEmpty()
                    && !nome.toLowerCase(Locale.ITALY).contains(cerca)
                    && !url.toLowerCase(Locale.ITALY).contains(cerca)) continue;
            if (!elencoPreferiti) {
                String g = giorno(o.optLong("t", 0));
                if (!g.equals(gruppo)) { gruppo = g; elencoCorpo.addView(intestazioneGiorno(g)); }
            }
            final int idx = i;
            final String indirizzo = url;
            View.OnClickListener azioni = v -> azioniVoce(idx);
            View riga = rigaElenco(nome, url, url, azioni);
            riga.setOnClickListener(v -> { chiudiElenco(); apriUrl(indirizzo, false); });
            elencoCorpo.addView(riga);
            mostrate++;
        }
        if (mostrate == 0) {
            TextView v = new TextView(this);
            v.setText(cerca.isEmpty()
                    ? (elencoPreferiti ? "Nessun preferito.\nUsa «Aggiungi ai preferiti» dal menu su una pagina."
                                       : "Nessuna cronologia.\nLe pagine che apri finiscono qui.")
                    : "Nessun risultato per «" + elencoFiltro.trim() + "».");
            v.setTextColor(colDim());
            v.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
            v.setLineSpacing(0, 1.2f);
            v.setGravity(Gravity.CENTER);
            v.setPadding(dp(32), dp(72), dp(32), 0);
            elencoCorpo.addView(v);
        }
    }

    /** «Oggi», «Ieri», o la data: l'intestazione che apre un giorno della cronologia. */
    private static String giorno(long quando) {
        if (quando <= 0) return "Meno recenti";
        java.util.Calendar c = java.util.Calendar.getInstance();
        c.set(java.util.Calendar.HOUR_OF_DAY, 0); c.set(java.util.Calendar.MINUTE, 0);
        c.set(java.util.Calendar.SECOND, 0); c.set(java.util.Calendar.MILLISECOND, 0);
        long oggi = c.getTimeInMillis();
        if (quando >= oggi) return "Oggi";
        if (quando >= oggi - 86400000L) return "Ieri";
        String s = new SimpleDateFormat("EEE d MMMM", Locale.ITALY).format(new java.util.Date(quando));
        return s.substring(0, 1).toUpperCase(Locale.ITALY) + s.substring(1);
    }

    private TextView intestazioneGiorno(String testo) {
        TextView h = new TextView(this);
        h.setText(testo);
        h.setTextColor(colDim());
        h.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        h.setPadding(dp(18), dp(18), dp(18), dp(6));
        return h;
    }

    /** Il titolo di una voce di cronologia: quello salvato, o il sito se non c'è. */
    private static String nomeCronologia(JSONObject o) {
        String n = o.optString("name", "");
        String url = o.optString("url", "");
        if (!n.isEmpty()) return n;
        String host = hostDi(url);
        return host.isEmpty() ? url : host;
    }

    /**
     * Una riga di un elenco: la tessera del sito, il titolo, il sito, e il ⋮ delle azioni.
     *
     * <p>La tessera porta l'iniziale del sito e non la sua icona: il browser non tiene un
     * archivio delle icone dei siti, e andarle a prendere una per una vorrebbe dire una
     * richiesta a un servizio esterno per ogni riga dell'elenco — cioè raccontare a qualcun
     * altro quali siti hai visitato. L'iniziale è nostra, il colore è nostro, e la riga si
     * legge lo stesso: a dire quale sito è, c'è scritto sotto il titolo.
     */
    private View rigaElenco(String nome, String url, String perTessera, final View.OnClickListener azioni) {
        final int cTxt = colTxt(), cDim = colDim();
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.HORIZONTAL);
        r.setGravity(Gravity.CENTER_VERTICAL);
        r.setPadding(dp(18), dp(9), dp(6), dp(9));
        r.setBackgroundResource(sfondoTocco());
        r.setLongClickable(true);

        String host = hostDi(url);
        TextView tessera = new TextView(this);
        tessera.setText(iniziale(host.isEmpty() ? perTessera : host));
        tessera.setTextColor(Color.WHITE);
        tessera.setTextSize(TypedValue.COMPLEX_UNIT_SP, 17);
        tessera.setGravity(Gravity.CENTER);
        GradientDrawable cerchio = new GradientDrawable();
        cerchio.setShape(GradientDrawable.OVAL);
        cerchio.setColor(tintaSito(host));
        tessera.setBackground(cerchio);
        LinearLayout.LayoutParams tlp = new LinearLayout.LayoutParams(dp(38), dp(38));
        tlp.rightMargin = dp(14);
        tessera.setLayoutParams(tlp);
        r.addView(tessera);

        LinearLayout testo = new LinearLayout(this);
        testo.setOrientation(LinearLayout.VERTICAL);
        testo.setLayoutParams(new LinearLayout.LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f));
        TextView t = new TextView(this);
        t.setText(nome == null || nome.isEmpty() ? url : nome);
        t.setTextColor(cTxt);
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        t.setSingleLine(true);
        t.setEllipsize(TextUtils.TruncateAt.END);
        TextView u = new TextView(this);
        u.setText(host.isEmpty() ? url : host);
        u.setTextColor(cDim);
        u.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        u.setSingleLine(true);
        u.setEllipsize(TextUtils.TruncateAt.END);
        testo.addView(t); testo.addView(u);
        r.addView(testo);

        Button piu = new Button(this);
        piu.setText("⋮");
        piu.setAllCaps(false);
        piu.setBackgroundColor(Color.TRANSPARENT);
        piu.setTextColor(cDim);
        piu.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        piu.setPadding(0, 0, 0, 0);
        piu.setMinWidth(dp(44)); piu.setMinimumWidth(dp(44));
        piu.setLayoutParams(new LinearLayout.LayoutParams(dp(44), dp(44)));
        // Le azioni si aprono da qui e dal dito tenuto premuto sulla riga: il ⋮ si vede, e
        // chi non lo guarda preme a lungo la riga come in un elenco qualsiasi.
        piu.setOnClickListener(azioni);
        r.setOnLongClickListener(v -> { azioni.onClick(v); return true; });
        r.addView(piu);
        return r;
    }

    /** Le azioni di una voce dell'elenco, nel foglio dal basso. */
    private void azioniVoce(final int idx) {
        JSONArray a = arr(elencoPreferiti ? "bookmarks" : "browserHistory");
        final JSONObject o = a.optJSONObject(idx);
        if (o == null) return;
        final String url = o.optString("url", "");
        final String nome = elencoPreferiti ? nomePreferito(o) : nomeCronologia(o);
        LinearLayout corpo = apriFoglio(nome, "Annulla");
        corpo.addView(rigaFoglio("Apri", null, () -> { chiudiElenco(); apriUrl(url, false); }));
        corpo.addView(rigaFoglio("Apri in una nuova scheda", null, () -> { chiudiElenco(); apriUrl(url, true); }));
        if (elencoPreferiti) corpo.addView(rigaFoglio("Rinomina", null, () -> rinominaPreferito(idx)));
        corpo.addView(rigaFoglio(elencoPreferiti ? "Elimina" : "Rimuovi dalla cronologia", null, () -> {
            if (elencoPreferiti) {
                rimuoviPreferitoIndice(idx);
                Toast.makeText(this, "Preferito eliminato", Toast.LENGTH_SHORT).show();
            } else {
                rimuoviCronologiaIndice(idx);
                Toast.makeText(this, "Tolto dalla cronologia", Toast.LENGTH_SHORT).show();
            }
            riempiElenco();
        }));
    }

    /** Apre un indirizzo nella scheda in vista o in una nuova. */
    private void apriUrl(String url, boolean nuovaScheda_) {
        if (url == null || url.isEmpty()) return;
        if (nuovaScheda_) { nuovaScheda(url, false, false); return; }
        Scheda s = schedaCorrente();
        if (s == null || s.web == null) nuovaScheda(url, false, false);
        else s.web.loadUrl(url);
    }

    private void rimuoviCronologiaIndice(int idx) {
        try {
            JSONArray a = arr("browserHistory"), out = new JSONArray();
            for (int i = 0; i < a.length(); i++) if (i != idx) out.put(a.get(i));
            putArr("browserHistory", out);
        } catch (Exception e) { Log.w(TAG, "cronologia non aggiornata", e); }
    }

    /** La prima lettera del sito, quella che sta nella tessera. */
    private static String iniziale(String host) {
        if (host == null || host.isEmpty()) return "•";
        String h = host.startsWith("www.") ? host.substring(4) : host;
        return h.isEmpty() ? "•" : h.substring(0, 1).toUpperCase(Locale.ITALY);
    }

    /**
     * Il colore della tessera di un sito: sempre lo stesso per lo stesso sito.
     *
     * <p>Si ricava dal nome con una somma pesata — non da un numero casuale, che
     * cambierebbe colore a ogni disegno, e nemmeno da un colore preso dal sito, che sarebbe
     * un colore che non si vede su un fondo chiaro o su uno scuro.
     */
    private static int tintaSito(String host) {
        if (host == null || host.isEmpty()) return 0xFF6b7280;
        int h = 0;
        for (int i = 0; i < host.length(); i++) h = h * 31 + host.charAt(i);
        return TINTE[Math.abs(h % TINTE.length)];
    }

    /** I colori delle tessere: pieni, scuri abbastanza da reggere una lettera bianca. */
    private static final int[] TINTE = {
        0xFF4c6ef5, 0xFF0c8599, 0xFFe8590c, 0xFF7048e8, 0xFF2b8a3e,
        0xFFc2255c, 0xFF1971c2, 0xFFa61e4d, 0xFF5f3dc4, 0xFF087f5b,
    };

    /** Rinomina un preferito: un avviso con il campo del nome. */
    private void rinominaPreferito(final int idx) {
        JSONArray a = arr("bookmarks");
        JSONObject o = a.optJSONObject(idx);
        if (o == null) return;
        final EditText in = new EditText(this);
        in.setText(nomePreferito(o));
        in.setSelectAllOnFocus(true);
        in.setTextColor(colTxt());
        in.setHintTextColor(colDim());
        in.setSingleLine(true);
        in.setInputType(InputType.TYPE_CLASS_TEXT);
        in.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        LinearLayout box = new LinearLayout(this);
        box.setPadding(0, dp(12), 0, 0);
        box.addView(in);
        mostraAvviso("Rinomina preferito", null, box,
            Pulsante.normale("Annulla", null),
            Pulsante.principale("Salva", () -> {
                try {
                    JSONArray b = arr("bookmarks");
                    JSONObject bo = b.optJSONObject(idx);
                    if (bo != null) {
                        String t = in.getText().toString().trim();
                        bo.put("name", t.isEmpty() ? bo.optString("url") : t);
                        putArr("bookmarks", b);
                        preferitiCambiati();
                        riempiElenco();
                    }
                } catch (Exception e) { Log.w(TAG, "rinomina non salvata", e); }
            }));
    }

    /**
     * Elimina la cronologia, e se si vuole anche il resto.
     *
     * <p>La cronologia è la voce, e si cancella da sola; cookie, dati dei siti e cache
     * stanno nella stessa casella perché sono la stessa domanda — «cosa resta sul telefono
     * di dove sono stato» — ma si cancellano solo se li si chiede: sono anche le cose che
     * tengono l'accesso fatto ai siti, e cancellarli senza dirlo sarebbe un dispetto.
     */
    private void eliminaCronologia() {
        final CheckBox anche = new CheckBox(this);
        anche.setText("Elimina anche cookie, dati dei siti e cache");
        anche.setTextColor(colTxt());
        anche.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        anche.setPadding(0, dp(14), 0, 0);
        mostraAvviso("Elimina cronologia", "Le pagine visitate non saranno più nell'elenco. I preferiti non si toccano.", anche,
            Pulsante.normale("Annulla", null),
            Pulsante.principale("Elimina", () -> {
                putArr("browserHistory", new JSONArray());
                if (anche.isChecked()) cancellaDatiDiNavigazione();
                if (elenco != null && !elencoPreferiti) riempiElenco();
                Toast.makeText(this, anche.isChecked()
                        ? "Cronologia, cookie e cache eliminati" : "Cronologia eliminata", Toast.LENGTH_SHORT).show();
            }));
    }

    /** Cookie, dati dei siti e cache: quello che rende «riconoscibile» un telefono. */
    private void cancellaDatiDiNavigazione() {
        try {
            CookieManager cm = CookieManager.getInstance();
            cm.removeAllCookies(null);
            cm.flush();
        } catch (Exception e) { Log.w(TAG, "cookie non cancellati", e); }
        try { WebStorage.getInstance().deleteAllData(); } catch (Exception e) { Log.w(TAG, "dati non cancellati", e); }
        for (Scheda t : schede) {
            if (t.web == null) continue;
            try { t.web.clearCache(true); t.web.clearFormData(); } catch (Exception e) { /* niente da fare */ }
        }
    }

    // =====================================================================
    //  Da una pagina a una cosa che resta: la lettura, un'app nella home
    // =====================================================================

    /**
     * Il valore che una pagina ha restituito a {@code evaluateJavascript}, decodificato.
     *
     * <p>La risposta arriva come una stringa JSON che contiene il valore, non come il
     * valore: se la pagina restituisce una stringa, alla fine ci sono due strati di
     * virgolette e di barre rovesciate. Toglierli a mano funziona finché dentro non c'è una
     * virgoletta; leggerli come JSON funziona sempre.
     */
    private static String testoDaJs(String risposta) {
        if (risposta == null) return "";
        try {
            Object v = new JSONTokener(risposta).nextValue();
            return v == null ? "" : v.toString();
        } catch (Exception e) { return ""; }
    }

    /**
     * Il testo della pagina, ricavato dalla pagina stessa.
     *
     * <p>Il criterio è quello di sempre nei lettori: il pezzo di pagina con più caratteri
     * <i>suoi</i> — cioè non dentro un collegamento — e con almeno qualche capoverso. La
     * densità di collegamenti è quello che distingue un articolo da un menu, da una lista di
     * risultati o da una pagina di indice: sono tutti pieni di testo, ma quasi tutto il loro
     * testo è cliccabile. Vince il pezzo col punteggio più alto, e di quel pezzo si tiene
     * l'HTML, ripulito da script, moduli, testate e piè di pagina e da ogni attributo che
     * non sia un indirizzo.
     *
     * <p>Se un articolo non c'è, la pagina lo dice: «non c'è un testo da estrarre» è una
     * risposta, una pagina bianca sarebbe un guasto.
     */
    private static final String JS_LETTURA =
          "(function(){try{"
        + "var via='script,style,noscript,iframe,svg,form,nav,aside,footer,header,button,input,select,textarea,[role=navigation],[aria-hidden=true]';"
        + "function testo(e){return (e.innerText||'').trim();}"
        + "var tutti=document.querySelectorAll('article,main,[role=main],section,div,td'),cand=[];"
        + "for(var i=0;i<tutti.length;i++){var e=tutti[i];var t=testo(e);if(t.length<400)continue;"
        + "if(e.getElementsByTagName('p').length<3)continue;"
        + "var link=0,aa=e.getElementsByTagName('a');for(var j=0;j<aa.length;j++)link+=testo(aa[j]).length;"
        + "cand.push({e:e,p:t.length*(t.length-link)/t.length});}"
        + "if(!cand.length)return JSON.stringify({ok:false,motivo:'in questa pagina non c\\u2019è un testo da estrarre'});"
        + "cand.sort(function(a,b){return b.p-a.p;});"
        + "var cl=cand[0].e.cloneNode(true),brutti=cl.querySelectorAll(via);"
        + "for(var k=brutti.length-1;k>=0;k--)brutti[k].parentNode.removeChild(brutti[k]);"
        + "var nodi=cl.querySelectorAll('*');"
        + "for(var k=0;k<nodi.length;k++){var n=nodi[k],at=n.attributes;"
        + "for(var q=at.length-1;q>=0;q--){var nome=at[q].name;"
        + "if(nome!=='href'&&nome!=='src'&&nome!=='alt'&&nome!=='colspan'&&nome!=='rowspan')n.removeAttribute(nome);}"
        + "if(n.tagName==='IMG')n.setAttribute('loading','lazy');}"
        + "var h=document.querySelector('h1'),titolo=(h&&testo(h))?testo(h):(document.title||'').trim();"
        + "return JSON.stringify({ok:true,titolo:titolo,html:cl.innerHTML});"
        + "}catch(e){return JSON.stringify({ok:false,motivo:'questa pagina non si è lasciata leggere'});}})()";

    /** «Mostra modalità lettura»: chiede alla pagina il suo testo e lo apre da solo. */
    private void modalitaLettura() {
        final Scheda t = schedaCorrente();
        if (t == null || t.web == null || t.indirizzo.isEmpty()) return;
        if (t.lettura) { Toast.makeText(this, "Questa scheda è già in modalità lettura", Toast.LENGTH_SHORT).show(); return; }
        t.web.evaluateJavascript(JS_LETTURA, r -> {
            String testo = testoDaJs(r);
            try {
                JSONObject o = new JSONObject(testo);
                if (!o.optBoolean("ok")) {
                    Toast.makeText(this, o.optString("motivo", "Non c'è un testo da leggere"), Toast.LENGTH_LONG).show();
                    return;
                }
                apriLettura(t, o.optString("titolo", ""), o.optString("html", ""));
            } catch (Exception e) {
                Log.w(TAG, "lettura non riuscita", e);
                Toast.makeText(this, "Non sono riuscito a leggere questa pagina", Toast.LENGTH_SHORT).show();
            }
        });
    }

    /**
     * Apre il testo estratto in una scheda sua.
     *
     * <p>In una scheda a parte e non al posto della pagina: così la pagina vera resta dov'è,
     * con la sua impaginazione e i suoi collegamenti, e si torna a leggerla chiudendo
     * questa. L'indirizzo resta quello del sito (è la base della pagina, e serve anche a
     * fare arrivare le immagini e a far funzionare i collegamenti relativi), ma la scheda
     * sa di essere una lettura: non finisce in cronologia.
     *
     * <p>La pagina è nostra, quindi la scriviamo col tema di NovaOS: fondo, testo e
     * collegamenti del sistema, colonna larga al massimo 34 em (oltre, l'occhio salta di
     * riga in riga) e immagini mai più larghe dello schermo.
     */
    private void apriLettura(Scheda origine, String titolo, String html) {
        if (html == null || html.trim().isEmpty()) {
            Toast.makeText(this, "Non c'è un testo da leggere", Toast.LENGTH_SHORT).show();
            return;
        }
        boolean inc = origine.incognito;
        String cTxt = inc ? "#eceaf4" : esadecimale(TXT);
        String cDim = inc ? "#b3aec6" : esadecimale(DIM);
        String cBg  = inc ? "#17141f" : esadecimale(BG);
        String cSup = inc ? "#231d33" : esadecimale(CARD);
        String tit = titolo == null || titolo.trim().isEmpty() ? hostDi(origine.indirizzo) : titolo.trim();
        String css =
              "html,body{margin:0;padding:0}"
            + "body{background:" + cBg + ";color:" + cTxt + ";font-size:17px;line-height:1.68;"
            + "padding:22px 20px 48px;word-wrap:break-word}"
            + "h1{font-size:25px;line-height:1.28;margin:0 0 8px}"
            + ".fonte{color:" + cDim + ";font-size:13px;margin:0 0 26px}"
            + "h2,h3,h4{line-height:1.35;margin:1.5em 0 .5em}"
            + "p{margin:0 0 1.05em}"
            + "img,video{max-width:100%;height:auto;border-radius:8px}"
            + "figure{margin:0 0 1.2em}figcaption{color:" + cDim + ";font-size:13px}"
            + "a{color:" + esadecimale(ACC) + ";text-decoration:none}"
            + "blockquote{margin:0 0 1.1em;padding:2px 0 2px 16px;border-left:3px solid " + cDim + "}"
            + "pre,code{white-space:pre-wrap;word-wrap:break-word;background:" + cSup + ";border-radius:8px;"
            + "font-size:15px;padding:1px 5px}pre{padding:12px}"
            + "table{max-width:100%;border-collapse:collapse;font-size:15px}"
            + "th,td{border:1px solid " + cDim + ";padding:6px 8px;text-align:left}"
            + "hr{border:0;border-top:1px solid " + cDim + ";margin:1.6em 0}";
        String pagina = "<!doctype html><html lang=\"it\"><head><meta charset=\"utf-8\">"
            + "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">"
            + "<title>" + TextUtils.htmlEncode(tit) + "</title><style>" + css + "</style></head><body>"
            + "<h1>" + TextUtils.htmlEncode(tit) + "</h1>"
            + "<p class=\"fonte\">" + TextUtils.htmlEncode(hostDi(origine.indirizzo)) + " · modalità lettura</p>"
            + html + "</body></html>";

        Scheda l = nuovaScheda(null, inc, false);
        l.lettura = true;
        l.titolo = tit;
        if (l.web != null) l.web.loadDataWithBaseURL(origine.indirizzo, pagina, "text/html", "utf-8", null);
    }

    private static String esadecimale(int colore) {
        return String.format("#%06x", colore & 0xFFFFFF);
    }

    /**
     * Quello che la pagina dichiara di sé: se è un'app installabile (il suo manifest) e che
     * icona ha. È una sola lettura del documento, quella che serve a tutte e due le voci.
     */
    private static final String JS_PAGINA =
          "(function(){try{"
        + "var l=document.querySelector('link[rel=\"manifest\"]'),f=document.querySelector('link[rel~=\"icon\"]'),"
        + "t=document.querySelector('meta[name=\"theme-color\"]');"
        + "return JSON.stringify({manifest:(l&&l.href)?l.href:'',favicon:(f&&f.href)?f.href:'',"
        + "titolo:(document.title||'').trim(),tema:(t&&t.content)?t.content:''});"
        + "}catch(e){return JSON.stringify({manifest:'',favicon:'',titolo:'',tema:''});}})()";

    /**
     * «Installa»: la pagina diventa un'app della home di NovaOS.
     *
     * <p>La differenza con «Crea scorciatoia» è tutta in quello che la pagina dichiara: qui
     * serve il manifest, il file in cui una pagina dice di essere un'applicazione — nome
     * corto, colore, icona — ed è quello che si porta dietro nella home. Senza manifest non
     * si installa niente, e si dice perché: un'icona identica a una scorciatoia che finge di
     * essere un'app sarebbe una bugia.
     */
    private void installaApp() {
        final Scheda t = schedaCorrente();
        if (t == null || t.web == null || t.indirizzo.isEmpty()) return;
        t.web.evaluateJavascript(JS_PAGINA, r -> {
            JSONObject o = leggiJs(r);
            final String manifest = o.optString("manifest", "");
            final String titolo = o.optString("titolo", "");
            final String favicon = o.optString("favicon", "");
            if (manifest.isEmpty()) {
                mostraAvviso("Installa", "Questa pagina non dichiara un'applicazione: non dichiara cioè il file (il «manifest») con il nome, il colore e l'icona con cui presentarsi nella home.\n\nPuoi aggiungerla come scorciatoia: si aprirà lo stesso, con il titolo e l'icona della pagina.", null,
                    Pulsante.normale("Chiudi", null),
                    Pulsante.principale("Crea scorciatoia", this::creaScorciatoia));
                return;
            }
            leggiManifest(manifest, titolo, favicon, t.indirizzo);
        });
    }

    /**
     * Legge il manifest dell'app e la installa.
     *
     * <p>La lettura va fuori dal filo dell'interfaccia: è una richiesta di rete, e la rete
     * può metterci secondi o non rispondere mai. Con i cookie della pagina, perché il
     * manifest può stare dietro un accesso come tutto il resto.
     */
    private void leggiManifest(final String manifestUrl, final String titoloPagina,
                               final String favicon, final String paginaUrl) {
        new Thread(() -> {
            String nome = "", colore = "", icona = "";
            try {
                java.net.HttpURLConnection c = (java.net.HttpURLConnection) apri(new java.net.URL(manifestUrl));
                c.setConnectTimeout(8000);
                c.setReadTimeout(8000);
                String ua = mobileUa != null ? mobileUa : "Mozilla/5.0 (Linux; Android) NovaOS";
                c.setRequestProperty("User-Agent", ua);
                String cookie = CookieManager.getInstance().getCookie(manifestUrl);
                if (cookie != null) c.setRequestProperty("Cookie", cookie);
                java.io.InputStream in = c.getInputStream();
                java.io.ByteArrayOutputStream fuori = new java.io.ByteArrayOutputStream();
                byte[] buf = new byte[8192];
                int letti, tot = 0;
                // 512 kB: un manifest più grande di così non è un manifest.
                while ((letti = in.read(buf)) > 0 && tot < 512 * 1024) { fuori.write(buf, 0, letti); tot += letti; }
                in.close();
                JSONObject m = new JSONObject(fuori.toString("UTF-8"));
                nome = m.optString("short_name", "");
                if (nome.isEmpty()) nome = m.optString("name", "");
                colore = coloreSicuro(m.optString("theme_color", ""));
                icona = sceltoIcona(m.optJSONArray("icons"), manifestUrl);
            } catch (Exception e) {
                // Manifest illeggibile: si installa lo stesso, con nome e icona della pagina.
                Log.i(TAG, "manifest non leggibile: " + manifestUrl, e);
            }
            final String fn = nome, fc = colore, fi = icona;
            runOnUiThread(() -> {
                String nomeFinale = !fn.isEmpty() ? fn : (titoloPagina.isEmpty() ? hostDi(paginaUrl) : titoloPagina);
                installaInShell(nomeFinale, paginaUrl, fc, fi.isEmpty() ? favicon : fi);
            });
        }, "manifest-app").start();
    }

    /**
     * L'icona da usare fra quelle dichiarate: la prima delle misure che servono davvero a
     * un'icona della home, altrimenti la prima che c'è.
     */
    private static String sceltoIcona(JSONArray icone, String base) {
        if (icone == null) return "";
        String[] misure = { "192", "512", "180", "144", "128", "96", "256" };
        for (String m : misure) {
            for (int i = 0; i < icone.length(); i++) {
                JSONObject ic = icone.optJSONObject(i);
                if (ic == null) continue;
                if (ic.optString("sizes", "").contains(m)) {
                    String src = assoluto(ic.optString("src", ""), base);
                    if (!src.isEmpty()) return src;
                }
            }
        }
        for (int i = 0; i < icone.length(); i++) {
            JSONObject ic = icone.optJSONObject(i);
            if (ic == null) continue;
            String src = assoluto(ic.optString("src", ""), base);
            if (!src.isEmpty() && (src.startsWith("http://") || src.startsWith("https://"))) return src;
        }
        return "";
    }

    /** Un indirizzo relativo risolto rispetto al file che lo contiene. */
    private static String assoluto(String src, String base) {
        if (src == null || src.isEmpty()) return "";
        try { return java.net.URI.create(base).resolve(src).toString(); } catch (Exception e) { return src; }
    }

    /**
     * Un colore accettabile come sfondo di un'icona: solo {@code #rgb}, {@code #rrggbb} o
     * {@code #rrggbbaa}, altrimenti niente.
     *
     * <p>Non è pignoleria: questo valore arriva da una pagina web e finisce dentro un
     * attributo {@code style} della home di NovaOS. Tutto quello che non è un colore — una
     * virgoletta, un «;», un «url(…)» — uscirebbe dall'attributo e diventerebbe markup
     * scritto da un sito dentro il launcher.
     */
    private static String coloreSicuro(String v) {
        if (v == null) return "";
        v = v.trim();
        return v.matches("#[0-9a-fA-F]{3}|#[0-9a-fA-F]{6}|#[0-9a-fA-F]{8}") ? v : "";
    }

    /** «Crea scorciatoia»: la pagina entra nella home col suo titolo e la sua icona. */
    private void creaScorciatoia() {
        final Scheda t = schedaCorrente();
        if (t == null || t.web == null || t.indirizzo.isEmpty()) return;
        t.web.evaluateJavascript(JS_PAGINA, r -> {
            JSONObject o = leggiJs(r);
            String nome = o.optString("titolo", "").trim();
            if (nome.isEmpty()) nome = hostDi(t.indirizzo);
            installaInShell(nome, t.indirizzo, coloreSicuro(o.optString("tema", "")), o.optString("favicon", ""));
        });
    }

    /** Il JSON restituito da una pagina, o un oggetto vuoto: la pagina può anche sbagliare. */
    private static JSONObject leggiJs(String risposta) {
        try { return new JSONObject(testoDaJs(risposta)); } catch (Exception e) { return new JSONObject(); }
    }

    /**
     * Aggiunge una web app alla home di NovaOS.
     *
     * <p>La scrittura è la stessa che fa la shell quando installa un'app dalla sua vetrina —
     * l'elenco {@code userApps} nella copia condivisa delle preferenze, con la stessa forma
     * di voce — e poi la shell viene avvisata, così l'icona compare subito invece che alla
     * prossima accensione. Il numero della voce è l'istante: è quello che fa la shell, ed è
     * l'unico modo di non ripetersi fra i due che scrivono lo stesso elenco.
     */
    private void installaInShell(String nome, String url, String colore, String icona) {
        if (url == null || !(url.startsWith("http://") || url.startsWith("https://"))) {
            Toast.makeText(this, "Questa pagina non si può aggiungere alla home", Toast.LENGTH_SHORT).show();
            return;
        }
        if (!url.startsWith("https://")) {
            // La home è del sistema: un'app che si apre in chiaro è una porta aperta.
            mostraAvviso("Aggiungi alla home",
                "Questa pagina viaggia in chiaro (http://): quello che si scrive dentro può essere letto da chi sta in mezzo.\n\nAggiungerla lo stesso?",
                null,
                Pulsante.normale("Annulla", null),
                Pulsante.principale("Aggiungi", () -> scriviApp(nome, url, colore, icona)));
            return;
        }
        scriviApp(nome, url, colore, icona);
    }

    private void scriviApp(String nome, String url, String colore, String icona) {
        try {
            JSONArray a = arr("userApps");
            long n = System.currentTimeMillis();
            while (esisteId(a, "web_" + n)) n++;
            JSONObject o = new JSONObject();
            o.put("id", "web_" + n);
            o.put("name", nome);
            o.put("url", url);
            o.put("color", colore == null || colore.isEmpty() ? "#6d8bff" : colore);
            // Il globo di serie, scritto come codice e non come disegno: nel file Java un
            // emoji è innocuo, ma qui la codifica del file non è una cosa che si vede.
            o.put("icon", icona == null || icona.isEmpty() ? "🌐" : icona);
            a.put(o);
            putArr("userApps", a);
            appInstallate();
            Toast.makeText(this, nome + " è nella home di NovaOS", Toast.LENGTH_SHORT).show();
        } catch (Exception e) {
            Log.w(TAG, "app non aggiunta alla home", e);
            Toast.makeText(this, "Non sono riuscito ad aggiungerla alla home", Toast.LENGTH_SHORT).show();
        }
    }

    private static boolean esisteId(JSONArray a, String id) {
        for (int i = 0; i < a.length(); i++) {
            JSONObject o = a.optJSONObject(i);
            if (o != null && id.equals(o.optString("id"))) return true;
        }
        return false;
    }

    // =====================================================================
    //  Le impostazioni del sito
    // =====================================================================

    /** L'host di un indirizzo, o la stringa vuota: è la chiave di tutto ciò che è «per sito». */
    private static String hostDi(String url) {
        if (url == null) return "";
        try {
            String h = Uri.parse(url).getHost();
            return h == null ? "" : h.toLowerCase(Locale.ITALY);
        } catch (Exception e) { return ""; }
    }

    private JSONObject sitiSalvati() {
        try { return new JSONObject(prefs().getString(PREF_SITI, "{}")); } catch (Exception e) { return new JSONObject(); }
    }

    /**
     * La decisione presa per un sito, o la stringa vuota se non se ne è presa nessuna.
     *
     * <p>Vuoto vuol dire «come sempre»: è il valore che non si salva, ed è per questo che il
     * ritorno al comportamento normale è la cancellazione della voce e non la scrittura di
     * un valore uguale a quello di partenza.
     */
    private String decisione(String host, String chiave) {
        if (host == null || host.isEmpty()) return "";
        JSONObject s = sitiSalvati().optJSONObject(host);
        return s == null ? "" : s.optString(chiave, "");
    }

    private void decidi(String host, String chiave, String valore) {
        if (host == null || host.isEmpty()) return;
        try {
            JSONObject tutti = sitiSalvati();
            JSONObject s = tutti.optJSONObject(host);
            if (s == null) s = new JSONObject();
            if (valore == null || valore.isEmpty()) s.remove(chiave); else s.put(chiave, valore);
            if (s.length() == 0) tutti.remove(host); else tutti.put(host, s);
            prefs().edit().putString(PREF_SITI, tutti.toString()).apply();
        } catch (Exception e) { Log.w(TAG, "decisione non salvata", e); }
    }

    /**
     * «Impostazioni del sito»: che cosa questo sito può fare e che cosa ha lasciato qui.
     *
     * <p>Sono le voci che il browser sa davvero mantenere. La fotocamera, il microfono e la
     * posizione si possono bloccare — e bloccarli vuol dire che la pagina non li riceve e
     * non c'è niente da rispondere; il permesso di Android, invece, si chiede alla pagina
     * quando serve, come si è sempre fatto. I pop-up si aprono in una scheda nuova: qui si
     * può dire di non aprirli. La vista desktop è l'unica voce che è anche un gusto, e vale
     * per il sito, non per la scheda: è il motivo per cui sta qui e non nel menu.
     */
    private void impostazioniSito() {
        final Scheda t = schedaCorrente();
        final String host = t == null ? "" : hostDi(t.indirizzo);
        if (host.isEmpty()) {
            Toast.makeText(this, "Nessun sito da impostare: apri una pagina", Toast.LENGTH_SHORT).show();
            return;
        }
        // La misura di quanto un sito ha lasciato sul telefono non si può chiedere e avere
        // subito: la risposta arriva da un'altra parte del sistema. Si chiede, e il foglio si
        // compone quando è arrivata — sono millisecondi, e in cambio la riga dice una cifra
        // vera invece di un'ipotesi.
        datiSito(host, quanti -> { if (!isFinishing()) foglioImpostazioniSito(t, host, quanti); });
    }

    private void foglioImpostazioniSito(final Scheda t, final String host, long quanti) {
        final boolean inc = t.incognito;
        LinearLayout corpo = apriFoglio(host, "Chiudi");
        corpo.addView(rigaFoglio("Fotocamera", statoPermesso(host, "camera"), () -> scegliPermesso(host, "camera", "Fotocamera")));
        corpo.addView(rigaFoglio("Microfono", statoPermesso(host, "microfono"), () -> scegliPermesso(host, "microfono", "Microfono")));
        corpo.addView(rigaFoglio("Posizione", statoPermesso(host, "posizione"), () -> scegliPermesso(host, "posizione", "Posizione")));
        corpo.addView(rigaFoglio("Pop-up", "block".equals(decisione(host, "popup")) ? "Bloccati" : "Consentiti", () -> {
            boolean bloccati = "block".equals(decisione(host, "popup"));
            decidi(host, "popup", bloccati ? "" : "block");
            Toast.makeText(this, bloccati ? "I pop-up di " + host + " si aprono" : "I pop-up di " + host + " sono bloccati", Toast.LENGTH_SHORT).show();
            impostazioniSito();
        }));
        corpo.addView(rigaFoglio("Sito desktop", t.desktop ? "Sì" : "No", () -> {
            cambiaModalita(t, !t.desktop);
            impostazioniSito();
        }));
        corpo.addView(rigaFoglio("Cookie e dati del sito", riassuntoDati(host, quanti), () -> dettagliDati(host, quanti)));
        if (inc) {
            TextView n = new TextView(this);
            n.setText("In incognito non si salva niente: alla chiusura della scheda cookie e dati di questa sessione spariscono.");
            n.setTextColor(colDim());
            n.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
            n.setPadding(dp(22), dp(10), dp(22), dp(4));
            corpo.addView(n);
        }
    }

    private String statoPermesso(String host, String chiave) {
        return "block".equals(decisione(host, chiave)) ? "Bloccata" : "Chiedi";
    }

    private void scegliPermesso(final String host, final String chiave, String nome) {
        LinearLayout corpo = apriFoglio(nome + " · " + host, "Annulla");
        boolean bloccato = "block".equals(decisione(host, chiave));
        corpo.addView(rigaScelta("Chiedi alla pagina: la richiesta arriva, e decide Android", !bloccato, () -> {
            decidi(host, chiave, "");
            impostazioniSito();
        }));
        corpo.addView(rigaScelta("Blocca: la pagina non riceve il permesso, e non si chiede niente", bloccato, () -> {
            decidi(host, chiave, "block");
            impostazioniSito();
        }));
    }

    /** Quanti cookie e quanti dati ha lasciato un sito. */
    private String riassuntoDati(String host, long quanti) {
        int n = nomiCookie(host).length;
        if (n == 0 && quanti == 0) return "niente";
        String c = n == 0 ? "nessun cookie" : (n == 1 ? "1 cookie" : n + " cookie");
        return quanti > 0 ? c + " · " + dimensione(quanti) : c;
    }

    private void dettagliDati(final String host, long quanti) {
        String[] nomi = nomiCookie(host);
        StringBuilder m = new StringBuilder();
        if (nomi.length == 0) m.append("Nessun cookie salvato.");
        else {
            m.append(nomi.length == 1 ? "Un cookie salvato:" : nomi.length + " cookie salvati:");
            for (int i = 0; i < nomi.length && i < 8; i++) m.append("\n· ").append(nomi[i]);
            if (nomi.length > 8) m.append("\n· …altri ").append(nomi.length - 8);
        }
        m.append("\n\n");
        m.append(quanti > 0
                ? "Dati del sito (memoria locale, database): " + dimensione(quanti) + "."
                : "Il sito non tiene dati suoi su questo telefono.");
        mostraAvviso(host, m.toString(), null,
            Pulsante.normale("Chiudi", null),
            Pulsante.principale("Cancella", () -> {
                cancellaDatiSito(host);
                Toast.makeText(this, "Cookie e dati di " + host + " cancellati", Toast.LENGTH_SHORT).show();
            }));
    }

    /** I nomi dei cookie che il sito ha lasciato: si leggono dall'intestazione dei cookie. */
    private String[] nomiCookie(String host) {
        if (host == null || host.isEmpty()) return new String[0];
        List<String> nomi = new ArrayList<>();
        for (String schema : new String[]{ "https://", "http://" }) {
            try {
                String tutti = CookieManager.getInstance().getCookie(schema + host);
                if (tutti == null) continue;
                for (String pezzo : tutti.split(";")) {
                    int uguale = pezzo.indexOf('=');
                    String nome = (uguale > 0 ? pezzo.substring(0, uguale) : pezzo).trim();
                    if (!nome.isEmpty() && !nomi.contains(nome)) nomi.add(nome);
                }
            } catch (Exception e) { /* niente cookie leggibili */ }
        }
        return nomi.toArray(new String[0]);
    }

    /**
     * Quanti byte ha lasciato un sito su questo telefono, sommando le due origini.
     *
     * <p>Si chiede due volte — una per l'indirizzo in chiaro e una per quello cifrato — e si
     * risponde quando hanno risposto tutte e due: la prima che arriva da sola non è il
     * totale, e un totale a metà sarebbe una cifra sbagliata.
     */
    private void datiSito(String host, final ValueCallback<Long> quando) {
        final long[] tot = { 0 };
        final int[] restanti = { 2 };
        for (String schema : new String[]{ "https://", "http://" }) {
            try {
                WebStorage.getInstance().getUsageForOrigin(schema + host, valore -> {
                    if (valore != null && valore > 0) tot[0] += valore;
                    if (--restanti[0] == 0) quando.onReceiveValue(tot[0]);
                });
            } catch (Exception e) {
                if (--restanti[0] == 0) quando.onReceiveValue(tot[0]);
            }
        }
    }

    /**
     * Cancella cookie e dati di un sito.
     *
     * <p>I cookie si cancellano uno per uno, col nome che hanno: non c'è una chiamata che
     * cancelli «i cookie di un sito», e cancellarli tutti toglierebbe l'accesso anche agli
     * altri siti. Si prova sia con l'indirizzo com'è sia con la versione senza «www», perché
     * un sito può aver scritto i suoi cookie in tutti e due i modi. I dati (memoria locale e
     * database) hanno invece la cancellazione per origine, ed è esatta.
     */
    private void cancellaDatiSito(String host) {
        try {
            CookieManager cm = CookieManager.getInstance();
            List<String> nomi = new ArrayList<>();
            for (String h : new String[]{ host, host.startsWith("www.") ? host.substring(4) : "www." + host }) {
                for (String nome : nomiCookie(h)) if (!nomi.contains(nome)) nomi.add(nome);
            }
            for (String h : new String[]{ host, host.startsWith("www.") ? host.substring(4) : "www." + host }) {
                for (String schema : new String[]{ "https://", "http://" }) {
                    for (String nome : nomi) cm.setCookie(schema + h, nome + "=; Max-Age=0; Path=/");
                }
            }
            cm.flush();
        } catch (Exception e) { Log.w(TAG, "cookie non cancellati", e); }
        try {
            for (String schema : new String[]{ "https://", "http://" }) WebStorage.getInstance().deleteOrigin(schema + host);
        } catch (Exception e) { Log.w(TAG, "dati non cancellati", e); }
    }

    // ------------------------------------------------------------------ pagina iniziale
    /**
     * La chiave della pagina iniziale, condivisa con la shell.
     *
     * <p>Sta nella stessa copia dei preferiti e del tema: la si cambia da qui e, se un
     * giorno la shell esporrà la voce nelle sue Impostazioni, la si cambia da lì, e resta
     * un valore solo. Il formato è la stringa JSON con cui la shell salva ogni preferenza.
     */
    private static final String PREF_HOME = "nova:browserHome";

    /**
     * Dove porta la casa: la pagina scelta dall'utente, o quella di serie.
     *
     * <p>Accetta sia un valore JSON ({@code "https://…"}) sia una stringa nuda, e uno
     * scritto senza schema ({@code wikipedia.org}) viene completato con {@code https://}:
     * la preferenza è leggibile e scrivibile anche a mano, e un valore scritto male deve
     * portare a una pagina, non a una ricerca.
     */
    private String paginaIniziale() {
        if (nyctalus) return HOME_NYCT;
        String v = prefs().getString(PREF_HOME, "");
        if (v == null) v = "";
        v = v.trim();
        if (v.length() >= 2 && v.charAt(0) == '"' && v.endsWith("\"")) {
            try { v = new JSONTokener(v).nextValue().toString().trim(); } catch (Exception e) { /* resta com'è */ }
        }
        if (v.isEmpty()) return HOME_PREDEFINITA;
        return v.matches("^[a-zA-Z][a-zA-Z0-9+.-]*:.*") ? v : "https://" + v;
    }

    private void vaiAllaHome() {
        Scheda s = schedaCorrente();
        String u = paginaIniziale();
        if (s == null || s.web == null) nuovaScheda(u, false, false);
        else s.web.loadUrl(u);
    }

    /**
     * Salva la pagina che si sta guardando come pagina iniziale.
     *
     * <p>Una pagina vuota non è una scelta: {@code about:blank} non è mai ciò che si vuole
     * trovarsi aprendo il browser, quindi la voce lo rifiuta invece di accettarlo e
     * lasciare una casa che non porta da nessuna parte.
     */
    private void impostaPaginaIniziale() {
        Scheda s = schedaCorrente();
        String u = s == null ? null : s.indirizzo;
        if (u == null || u.isEmpty() || u.startsWith("about:")) {
            Toast.makeText(this, "Apri prima una pagina da usare come pagina iniziale", Toast.LENGTH_SHORT).show();
            return;
        }
        prefs().edit().putString(PREF_HOME, JSONObject.quote(u)).apply();
        Toast.makeText(this, "Pagina iniziale impostata", Toast.LENGTH_SHORT).show();
    }

    // ------------------------------------------------------------------ condividi / sistema
    private void condividi(String url) {
        if (url == null || url.isEmpty()) return;
        Intent i = new Intent(Intent.ACTION_SEND);
        i.setType("text/plain");
        i.putExtra(Intent.EXTRA_TEXT, url);
        startActivity(Intent.createChooser(i, "Condividi"));
    }
    private void apriNelBrowserDiSistema(String url) {
        if (url == null || url.isEmpty()) return;
        try {
            Intent i = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
            i.addCategory(Intent.CATEGORY_BROWSABLE);
            i.setFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            startActivity(i);
        } catch (Exception e) {
            Toast.makeText(this, "Nessun browser disponibile sul dispositivo", Toast.LENGTH_SHORT).show();
        }
    }

    // ------------------------------------------------------------------ util UI
    private LinearLayout radice() {
        try { return (LinearLayout) ((ViewGroup) findViewById(android.R.id.content)).getChildAt(0); }
        catch (Exception e) { return null; }
    }

    /** Un pulsante della barra scritto con un carattere: frecce, stella, croce, più. */
    private Button iconBtn(String glyph) {
        Button b = new Button(this);
        b.setText(glyph);
        b.setAllCaps(false);
        b.setBackgroundColor(Color.TRANSPARENT);
        b.setTextColor(TXT);
        b.setTextSize(TypedValue.COMPLEX_UNIT_SP, 17);
        b.setMinWidth(dp(40)); b.setMinimumWidth(dp(40));
        b.setPadding(dp(6), 0, dp(6), 0);
        return b;
    }

    /**
     * Un'icona <b>disegnata</b>, di lato {@code lato} dp e del colore dato.
     *
     * <p>Il colore si dà qui e non nel file XML: lo stesso disegno serve la barra chiara e
     * quella scura, e il tema lo conosce solo la schermata — così c'è un disegno per icona
     * invece di uno per icona e per tema.
     */
    private ImageView iconaVista(int icona, int lato, int colore) {
        ImageView v = new ImageView(this);
        v.setImageResource(icona);
        v.setColorFilter(colore, PorterDuff.Mode.SRC_IN);
        v.setScaleType(ImageView.ScaleType.FIT_CENTER);
        v.setLayoutParams(new LinearLayout.LayoutParams(dp(lato), dp(lato)));
        return v;
    }

    /**
     * Un pulsante-icona della barra: la sola immagine, senza testo.
     *
     * <p>L'area di tocco resta 40 dp anche se il disegno ne occupa 24: è la misura sotto la
     * quale un dito comincia a sbagliare bersaglio, e il cerchio che si accende al tocco
     * deve starci dentro.
     */
    private ImageView iconaBtn(int icona) {
        ImageView v = iconaVista(icona, 40, TXT);
        v.setScaleType(ImageView.ScaleType.CENTER);
        // Cerchio che si accende al tocco, disegnato a mano invece di prendere
        // ?selectableItemBackgroundBorderless: quello porta con sé anche lo stato
        // «ha il fuoco», cioè un rettangolo verde che compare attorno all'icona appena
        // l'omnibox perde il fuoco — il fuoco va alla prima vista che lo accetta, che
        // nella barra è la casa. Lo stato che serve è uno solo: il dito che preme.
        GradientDrawable cerchio = new GradientDrawable();
        cerchio.setShape(GradientDrawable.OVAL);
        cerchio.setColor(Color.WHITE);
        v.setBackground(new RippleDrawable(ColorStateList.valueOf(0x33808080), null, cerchio));
        // E nemmeno il fuoco da tastiera: appena l'omnibox lo perde (naviga() lo lascia
        // andare) il fuoco va alla prima vista che lo accetta, e il cerchio resterebbe
        // acceso attorno alla casa come se fosse premuta. Sul telefono il fuoco da
        // tastiera non serve a nulla; l'area di tocco è la stessa cosa.
        v.setFocusable(false);
        return v;
    }

    private GradientDrawable squareBadge() { return squareBadge(DIM); }

    private GradientDrawable squareBadge(int colore) {
        GradientDrawable g = new GradientDrawable();
        g.setStroke(dp(2), colore); g.setCornerRadius(dp(5)); g.setColor(Color.TRANSPARENT);
        return g;
    }
    private int dp(int v) { return (int) (v * getResources().getDisplayMetrics().density + 0.5f); }
    private void hideKeyboard() {
        try { InputMethodManager im = (InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
            im.hideSoftInputFromWindow(omnibox.getWindowToken(), 0); } catch (Exception e) {}
    }

    @Override public boolean onKeyDown(int codice, KeyEvent evento) {
        // Tasto indietro fisico: stessa scala di precedenze di onBackPressed.
        if (codice == KeyEvent.KEYCODE_BACK) { onBackPressed(); return true; }
        return super.onKeyDown(codice, evento);
    }

    @Override public void onBackPressed() {
        // Prima le superfici nostre, dalla più esterna alla più interna: un avviso sopra un
        // elenco si chiude per primo, altrimenti il tasto indietro premuto due volte
        // chiuderebbe due cose diverse in un colpo solo.
        if (avviso != null) { chiudiAvviso(); return; }
        if (foglio != null) { chiudiFoglio(); return; }
        if (elenco != null) { chiudiElenco(); return; }
        if (menuAperto != null) { chiudiMenu(); return; }
        if (selettore != null) { chiudiSelettore(); return; }
        if (fullscreen) { setSchermoIntero(false); return; }
        if (findBar.getVisibility() == View.VISIBLE) { mostraBarraTrova(false); return; }
        if (tradBar != null && tradBar.getVisibility() == View.VISIBLE) { mostraBarraTraduzione(false); return; }
        Scheda s = schedaCorrente();
        if (s != null && s.web != null && s.web.canGoBack()) s.web.goBack();
        else if (schede.size() > 1) chiudiScheda(corrente);
        else super.onBackPressed();
    }

    /**
     * La chiusura del browser.
     *
     * <p>Le WebView si distruggono qui e non si lasciano al sistema: una WebView
     * dimenticata resta viva con la sua pagina, e in incognito con la sua cronologia.
     */
    @Override protected void onDestroy() {
        chiudiMenu();
        for (Scheda t : schede) {
            try { t.web.destroy(); } catch (Exception e) { /* già distrutta */ }
        }
        schede.clear();
        if (nyctalus) cancellaDatiNavigazione();
        super.onDestroy();
        // Processo separato (":browser"): chiudendolo si azzera anche il proxy della WebView,
        // così la prossima apertura parte sempre dallo stato normale.
        if (isFinishing()) android.os.Process.killProcess(android.os.Process.myPid());
    }

    // ------------------------------------------------------------------ schermo intero
    private boolean fullscreen = false;
    private ImageView fsRestore;   // pulsante flottante per tornare alla vista normale

    private void setSchermoIntero(boolean on) {
        fullscreen = on;
        bar.setVisibility(on ? View.GONE : View.VISIBLE);
        progress.setVisibility(View.GONE);
        View decor = getWindow().getDecorView();
        if (on) {
            decor.setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
            addRestoreButton();
        } else {
            decor.setSystemUiVisibility(View.SYSTEM_UI_FLAG_VISIBLE);
            if (fsRestore != null && fsRestore.getParent() != null) ((ViewGroup) fsRestore.getParent()).removeView(fsRestore);
            fsRestore = null;
        }
    }

    /**
     * Il pulsante flottante in basso a destra per uscire dallo schermo intero.
     *
     * <p>Il disegno è bianco su un cerchio nero semitrasparente, e i colori sono fissi —
     * non quelli del tema. Il pulsante sta sopra la pagina, di cui non si sa nulla: qualunque
     * colore preso dal tema può finire su una pagina dello stesso colore, mentre il nero
     * semitrasparente col disegno bianco dentro si vede su qualunque sfondo. Con il colore
     * del testo del tema chiaro (quasi nero) su questo cerchio il segno spariva.
     */
    private void addRestoreButton() {
        if (fsRestore != null) return;
        fsRestore = new ImageView(this);
        fsRestore.setImageResource(R.drawable.ic_fullscreen_exit);
        fsRestore.setColorFilter(Color.WHITE, PorterDuff.Mode.SRC_IN);
        fsRestore.setScaleType(ImageView.ScaleType.CENTER);
        fsRestore.setContentDescription("Esci dallo schermo intero");
        GradientDrawable bg = new GradientDrawable();
        bg.setShape(GradientDrawable.OVAL);
        bg.setColor(0xCC000000);
        fsRestore.setBackground(bg);
        fsRestore.setClickable(true);
        fsRestore.setOnClickListener(v -> setSchermoIntero(false));
        FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(dp(48), dp(48));
        lp.gravity = Gravity.BOTTOM | Gravity.END;
        lp.rightMargin = dp(14); lp.bottomMargin = dp(20);
        ((ViewGroup) findViewById(android.R.id.content)).addView(fsRestore, lp);
    }
}
