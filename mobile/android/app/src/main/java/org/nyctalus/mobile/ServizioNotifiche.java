// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
package org.nyctalus.mobile;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;
import android.os.SystemClock;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Notifiche dei messaggi con l'app chiusa, SENZA Google: un servizio in primo piano (icona
 * fissa «Nyctalus attivo») ascolta il servizio in Termux (/api/notifiche, solo 127.0.0.1)
 * e mostra una notifica per ogni messaggio o file in arrivo. Nessuno all'esterno sa quando
 * ricevi qualcosa. Toccando la notifica si apre quella conversazione.
 *
 * Se Termux viene fermato da Android, il servizio lo riaccende (al più ogni 5 minuti).
 */
public class ServizioNotifiche extends Service {

    static final String CANALE_ATTIVO = "attivo", CANALE_MESSAGGI = "messaggi", CANALE_CHIAMATE = "chiamate";
    static final int ID_ATTIVO = 2;
    /** True mentre l'app è in primo piano: lì i messaggi si vedono già, niente notifiche. */
    static volatile boolean appInPrimoPiano = false;
    private static volatile Thread ascolto;
    /** Messaggi non ancora visti, per contatto (si azzerano aprendo l'app). */
    static final Map<String, List<String>> inAttesa = new HashMap<>();

    static void avvia(Context c) {
        if (!c.getSharedPreferences(MainActivity.PREFS, MODE_PRIVATE).getBoolean("notifiche", true)) return;
        Intent i = new Intent(c, ServizioNotifiche.class);
        try {
            if (Build.VERSION.SDK_INT >= 26) c.startForegroundService(i);
            else c.startService(i);
        } catch (Exception ignorata) {
            // Android 12+ non permette di avviarlo dallo sfondo: si riprova alla prossima apertura
        }
    }

    static void ferma(Context c) {
        c.stopService(new Intent(c, ServizioNotifiche.class));
    }

    /** L'app è stata aperta: le notifiche dei messaggi non servono più. */
    static void azzera(Context c) {
        synchronized (inAttesa) { inAttesa.clear(); }
        NotificationManager nm = c.getSystemService(NotificationManager.class);
        if (nm == null) return;
        for (android.service.notification.StatusBarNotification s : nm.getActiveNotifications())
            if (s.getId() != ID_ATTIVO && s.getId() != 1) nm.cancel(s.getId());
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        NotificationManager nm = getSystemService(NotificationManager.class);
        Notification.Builder b;
        if (Build.VERSION.SDK_INT >= 26) {
            nm.createNotificationChannel(new NotificationChannel(CANALE_ATTIVO, "Nyctalus attivo",
                    NotificationManager.IMPORTANCE_MIN));
            NotificationChannel msg = new NotificationChannel(CANALE_MESSAGGI, "Messaggi",
                    NotificationManager.IMPORTANCE_HIGH);
            msg.setDescription("Nuovi messaggi e file nella chat Nyctalus");
            nm.createNotificationChannel(msg);
            b = new Notification.Builder(this, CANALE_ATTIVO);
        } else {
            b = new Notification.Builder(this);
        }
        Notification n = b.setSmallIcon(R.drawable.ic_notifica)
                .setContentTitle("Nyctalus attivo")
                .setContentText("Ricevi i messaggi anche con l'app chiusa")
                .setContentIntent(apri(null))
                .setOngoing(true).build();
        if (Build.VERSION.SDK_INT >= 34) startForeground(ID_ATTIVO, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING);
        else if (Build.VERSION.SDK_INT >= 29) startForeground(ID_ATTIVO, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC);
        else startForeground(ID_ATTIVO, n);

        if (ascolto == null || !ascolto.isAlive()) {
            ascolto = new Thread(this::ascolta, "nyctalus-notifiche");
            ascolto.start();
        }
        return START_STICKY;
    }

    /** Si resta collegati al flusso delle notifiche; se cade si riprova con attese crescenti. */
    private void ascolta() {
        String segreto = getSharedPreferences(MainActivity.PREFS, MODE_PRIVATE).getString("segreto", "");
        long attesa = 3000, ultimoRiavvio = 0;
        while (ascolto == Thread.currentThread()) {
            HttpURLConnection c = null;
            try {
                c = (HttpURLConnection) new URL(MainActivity.URL_BASE + "/api/notifiche").openConnection();
                c.setConnectTimeout(3000);
                c.setReadTimeout(60000);           // il servizio manda «{}» ogni 25 s
                c.setRequestProperty("Cookie", "nyct=" + segreto);
                c.setInstanceFollowRedirects(false);
                if (c.getResponseCode() != 200) throw new java.io.IOException("risposta " + c.getResponseCode());
                try (BufferedReader in = new BufferedReader(new InputStreamReader(c.getInputStream(), "UTF-8"))) {
                    String riga;
                    while ((riga = in.readLine()) != null) {
                        attesa = 3000;
                        if (riga.length() > 2) notifica(new org.json.JSONObject(riga));
                    }
                }
            } catch (Exception e) {
                // Termux fermato da Android? Lo si riaccende, ma non più di una volta ogni 5 minuti
                if (SystemClock.elapsedRealtime() - ultimoRiavvio > 300000) {
                    ultimoRiavvio = SystemClock.elapsedRealtime();
                    MainActivity.avviaInTermux(this, segreto);
                }
            } finally {
                if (c != null) c.disconnect();
            }
            try { Thread.sleep(attesa); } catch (InterruptedException e) { return; }
            attesa = Math.min(attesa * 2, 60000);
        }
    }

    private void notifica(org.json.JSONObject o) {
        String tipo = o.optString("tipo"), da = o.optString("da");
        if (("chiamata".equals(tipo) || "fine_chiamata".equals(tipo)) && da.matches("[a-z2-7]{56}\\.nyct")) {
            chiamata(tipo, da, o.optString("nome", da.substring(0, 10)), o.optBoolean("video"), o.optString("id"));
            return;
        }
        if (!("msg".equals(tipo) || "file".equals(tipo)) || !da.matches("[a-z2-7]{56}\\.nyct")) return;
        if (appInPrimoPiano) return;
        String nome = o.optString("nome", da.substring(0, 10));
        String testo = "msg".equals(tipo) ? o.optString("testo") : descriviFile(o.optString("mime"), o.optString("file"));
        List<String> righe;
        synchronized (inAttesa) {
            righe = inAttesa.get(da);
            if (righe == null) { righe = new ArrayList<>(); inAttesa.put(da, righe); }
            righe.add(testo);
            if (righe.size() > 6) righe.remove(0);
            righe = new ArrayList<>(righe);
        }
        Notification.Builder b = Build.VERSION.SDK_INT >= 26
                ? new Notification.Builder(this, CANALE_MESSAGGI) : new Notification.Builder(this);
        Notification.InboxStyle stile = new Notification.InboxStyle();
        for (String r : righe) stile.addLine(r);
        b.setSmallIcon(R.drawable.ic_notifica)
                .setContentTitle(nome)
                .setContentText(testo)
                .setStyle(stile)
                .setNumber(righe.size())
                .setCategory(Notification.CATEGORY_MESSAGE)
                .setAutoCancel(true)
                .setContentIntent(apri(da));
        if (Build.VERSION.SDK_INT < 26) b.setPriority(Notification.PRIORITY_HIGH).setDefaults(Notification.DEFAULT_ALL);
        NotificationManager nm = getSystemService(NotificationManager.class);
        nm.notify(1000 + (da.hashCode() & 0x7fffff), b.build());
    }

    /** Numero della notifica di chiamata di un contatto (la toglie anche chi risponde o rifiuta). */
    static int idChiamata(String da) {
        return 500000 + (da.hashCode() & 0x7ffff);
    }

    /** Chiamata in arrivo: notifica che squilla, con Rispondi (verde) e Rifiuta (rosso). */
    private void chiamata(String tipo, String da, String nome, boolean video, String idChiamata) {
        NotificationManager nm = getSystemService(NotificationManager.class);
        int id = idChiamata(da);
        if ("fine_chiamata".equals(tipo)) {
            nm.cancel(id);
            return;
        }
        if (appInPrimoPiano) return;              // app aperta: squilla già la schermata della chiamata
        if (Build.VERSION.SDK_INT >= 26) {
            NotificationChannel ch = new NotificationChannel(CANALE_CHIAMATE, "Chiamate", NotificationManager.IMPORTANCE_HIGH);
            ch.setDescription("Chiamate in arrivo nella chat Nyctalus");
            nm.createNotificationChannel(ch);
        }
        Notification.Builder b = Build.VERSION.SDK_INT >= 26
                ? new Notification.Builder(this, CANALE_CHIAMATE) : new Notification.Builder(this);
        b.setSmallIcon(R.drawable.ic_notifica)
                .setContentTitle((video ? "📹 " : "📞 ") + nome)
                .setContentText(video ? "videochiamata in arrivo" : "ti sta chiamando")
                .setCategory(Notification.CATEGORY_CALL)
                .setAutoCancel(true)
                .setTimeoutAfter(45000)
                .setContentIntent(apri(da))
                .setFullScreenIntent(apri(da), true);
        if (idChiamata != null && idChiamata.matches("[A-Za-z0-9]{1,32}")) {
            PendingIntent rispondi = rispondi(da, video), rifiuta = rifiuta(da, idChiamata);
            if (Build.VERSION.SDK_INT >= 31) {
                // lo stile ufficiale delle chiamate: pulsanti verde e rosso del sistema
                android.app.Person chi = new android.app.Person.Builder().setName(nome).setImportant(true).build();
                b.setStyle(Notification.CallStyle.forIncomingCall(chi, rifiuta, rispondi).setIsVideo(video));
            } else {
                b.addAction(new Notification.Action.Builder(null, colorato("Rifiuta", 0xFFE53935), rifiuta).build());
                b.addAction(new Notification.Action.Builder(null, colorato("Rispondi", 0xFF2E9E5B), rispondi).build());
            }
        }
        if (Build.VERSION.SDK_INT < 26) b.setPriority(Notification.PRIORITY_MAX).setDefaults(Notification.DEFAULT_ALL);
        nm.notify(id, b.build());
    }

    private static String descriviFile(String mime, String nome) {
        if (mime.startsWith("image/")) return "📷 Foto";
        if (mime.startsWith("video/")) return "🎬 Video";
        if (mime.startsWith("audio/")) return "🎵 Audio";
        return "📎 " + (nome.isEmpty() ? "File" : nome);
    }

    /** «Rispondi»: apre l'app sulla chiamata, che risponde da sola (con il video se è una videochiamata). */
    private PendingIntent rispondi(String da, boolean video) {
        Intent i = new Intent(this, MainActivity.class);
        i.setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_SINGLE_TOP);
        i.putExtra("chat", da);
        i.putExtra("rispondi", da);
        i.putExtra("video", video);
        return PendingIntent.getActivity(this, idChiamata(da) + 1, i,
                PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 23 ? PendingIntent.FLAG_IMMUTABLE : 0));
    }

    /** «Rifiuta»: senza aprire l'app (ricevitore RifiutaChiamata). */
    private PendingIntent rifiuta(String da, String idChiamata) {
        Intent i = new Intent(this, RifiutaChiamata.class);
        i.putExtra("da", da);
        i.putExtra("id", idChiamata);
        return PendingIntent.getBroadcast(this, idChiamata(da) + 2, i,
                PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 23 ? PendingIntent.FLAG_IMMUTABLE : 0));
    }

    private static CharSequence colorato(String testo, int colore) {
        android.text.SpannableString s = new android.text.SpannableString(testo);
        s.setSpan(new android.text.style.ForegroundColorSpan(colore), 0, testo.length(), 0);
        return s;
    }

    /** Apre l'app; con un indirizzo, direttamente su quella conversazione. */
    private PendingIntent apri(String da) {
        Intent i = new Intent(this, MainActivity.class);
        i.setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_SINGLE_TOP);
        if (da != null) i.putExtra("chat", da);
        int codice = da == null ? 0 : (da.hashCode() & 0x7fffff);
        return PendingIntent.getActivity(this, codice, i,
                PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 23 ? PendingIntent.FLAG_IMMUTABLE : 0));
    }

    @Override
    public void onDestroy() {
        Thread t = ascolto;
        ascolto = null;
        if (t != null) t.interrupt();
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
