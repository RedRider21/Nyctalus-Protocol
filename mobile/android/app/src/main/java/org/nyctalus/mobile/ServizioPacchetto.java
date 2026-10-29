// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
package org.nyctalus.mobile;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;

/**
 * Tiene sveglia l'app mentre Termux scarica il pacchetto.
 *
 * Quando Termux è in primo piano, Android congela le app rimaste dietro (da Android 12
 * in poi su molti telefoni): il ServitorePacchetto smetterebbe di rispondere e curl
 * resterebbe appeso. Un servizio in primo piano, con la sua notifica, lo impedisce.
 * Si chiude da solo appena il pacchetto è stato consegnato (o dopo 15 minuti).
 */
public class ServizioPacchetto extends Service {

    static volatile ServitorePacchetto attivo;
    private static final String CANALE = "pacchetto";

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        Notification.Builder b;
        if (Build.VERSION.SDK_INT >= 26) {
            NotificationManager nm = getSystemService(NotificationManager.class);
            nm.createNotificationChannel(new NotificationChannel(CANALE, "Installazione in Termux",
                    NotificationManager.IMPORTANCE_LOW));
            b = new Notification.Builder(this, CANALE);
        } else {
            b = new Notification.Builder(this);
        }
        Notification n = b.setSmallIcon(android.R.drawable.stat_sys_download)
                .setContentTitle("Nyctalus")
                .setContentText("Passo il pacchetto a Termux…")
                .setOngoing(true).build();
        if (Build.VERSION.SDK_INT >= 29) startForeground(1, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC);
        else startForeground(1, n);

        new Thread(() -> {
            // finché c'è un servitore vivo (anche uno nuovo, se si ritocca il pulsante)
            ServitorePacchetto sp;
            while ((sp = attivo) != null && sp.isAlive()) {
                try { Thread.sleep(1000); } catch (InterruptedException e) { break; }
            }
            stopForeground(true);
            stopSelf();
        }).start();
        return START_NOT_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
