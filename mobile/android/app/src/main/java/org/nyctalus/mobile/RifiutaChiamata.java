// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
package org.nyctalus.mobile;

import android.app.NotificationManager;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;

/**
 * Pulsante «Rifiuta» della notifica di una chiamata in arrivo: senza aprire l'app, chiede al
 * servizio Nyctalus in Termux di rifiutare (CALLNO al motore) e toglie la notifica.
 * Non esportato: lo può attivare solo la nostra notifica.
 */
public class RifiutaChiamata extends BroadcastReceiver {
    @Override
    public void onReceive(Context ctx, Intent intent) {
        final String da = intent.getStringExtra("da"), id = intent.getStringExtra("id");
        if (da == null || id == null || !da.matches("[a-z2-7]{56}\\.nyct") || !id.matches("[A-Za-z0-9]{1,32}")) return;
        ctx.getSystemService(NotificationManager.class).cancel(ServizioNotifiche.idChiamata(da));
        final String segreto = ctx.getSharedPreferences(MainActivity.PREFS, Context.MODE_PRIVATE).getString("segreto", "");
        final PendingResult fine = goAsync();           // la rete non si usa sul thread principale
        new Thread(() -> {
            HttpURLConnection c = null;
            try {
                c = (HttpURLConnection) new URL(MainActivity.URL_BASE + "/api/chiamata/rifiuta?da="
                        + URLEncoder.encode(da, "UTF-8") + "&id=" + URLEncoder.encode(id, "UTF-8")).openConnection();
                c.setRequestMethod("POST");
                c.setConnectTimeout(3000);
                c.setReadTimeout(8000);
                c.setRequestProperty("Cookie", "nyct=" + segreto);
                c.setDoOutput(true);
                c.getOutputStream().close();
                c.getResponseCode();
            } catch (Exception ignorata) {
                // servizio spento: la chiamata smette comunque di squillare da sola dopo 45 s
            } finally {
                if (c != null) c.disconnect();
                fine.finish();
            }
        }).start();
    }
}
