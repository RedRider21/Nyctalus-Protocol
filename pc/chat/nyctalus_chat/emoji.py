# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Le emoticon della chat, per categoria. Lo stesso elenco è in mobile/webapp/emoji.js
(si rigenera con: python3 -m nyctalus_chat.emoji > ../../mobile/webapp/emoji.js)."""

CATEGORIE = [
    ("🙂", "Faccine",
     "😀 😃 😄 😁 😆 😅 🤣 😂 🙂 🙃 😉 😊 😇 🥰 😍 🤩 😘 😗 😚 😙 😋 😛 😜 🤪 😝 🤑 🤗 🤭 🤫 🤔 "
     "🤐 🤨 😐 😑 😶 😏 😒 🙄 😬 😌 😔 😪 🤤 😴 😷 🤒 🤕 🤢 🤮 🥵 🥶 🥴 😵 🤯 🤠 🥳 😎 🤓 🧐 😕 "
     "😟 🙁 😮 😯 😲 😳 🥺 😦 😧 😨 😰 😥 😢 😭 😱 😖 😣 😞 😓 😩 😫 🥱 😤 😡 😠 🤬 😈 👿 💀 🤡 👻 👽 🤖 💩"),
    ("👍", "Gesti",
     "👍 👎 👌 🤌 ✌️ 🤞 🤟 🤘 🤙 👈 👉 👆 👇 ☝️ ✋ 🤚 🖐️ 🖖 👋 🤝 🙏 👏 🙌 👐 🤲 💪 ✍️ 💅 🤳 "
     "👀 👁️ 👂 👃 🧠 👄 🦷 👶 🧒 👦 👧 🧑 👨 👩 🧓 👴 👵 🙋 🙇 🤦 🤷 💁 🙆 🙅 🏃 🚶 💃 🕺"),
    ("❤️", "Cuori",
     "❤️ 🧡 💛 💚 💙 💜 🖤 🤍 🤎 💔 ❣️ 💕 💞 💓 💗 💖 💘 💝 💟 💋 💌 💐 🌹 🥀 🌷 🌸 💍 💎"),
    ("🐶", "Animali e natura",
     "🐶 🐱 🐭 🐹 🐰 🦊 🐻 🐼 🐨 🐯 🦁 🐮 🐷 🐸 🐵 🐔 🐧 🐦 🐤 🦆 🦅 🦉 🦇 🐺 🐗 🐴 🦄 🐝 🐛 🦋 "
     "🐌 🐞 🐜 🐢 🐍 🦎 🐙 🦑 🦀 🐠 🐟 🐬 🐳 🐋 🦈 🐊 🐘 🦒 🐄 🐑 🐐 🐕 🐈 🌵 🌲 🌳 🌴 🍀 🍁 🍂 "
     "🌻 🌼 🌞 🌝 🌛 ⭐ 🌟 ✨ ⚡ 🔥 🌈 ☀️ ⛅ ☁️ 🌧️ ⛈️ ❄️ ☃️ 🌊 💧"),
    ("🍕", "Cibo",
     "🍏 🍎 🍐 🍊 🍋 🍌 🍉 🍇 🍓 🍒 🍑 🥭 🍍 🥥 🥝 🍅 🍆 🥑 🥦 🥕 🌽 🌶️ 🥔 🥐 🍞 🥖 🧀 🥚 🍳 🥓 "
     "🥩 🍗 🍖 🌭 🍔 🍟 🍕 🥪 🌮 🌯 🥗 🍝 🍜 🍲 🍣 🍱 🍤 🍙 🍚 🍦 🍰 🎂 🍫 🍬 🍭 🍩 🍪 ☕ 🍵 🥤 "
     "🍺 🍻 🥂 🍷 🍸 🍹 🍾"),
    ("⚽", "Attività",
     "⚽ 🏀 🏈 ⚾ 🎾 🏐 🏉 🎱 🏓 🏸 🥊 🥋 ⛳ 🎣 🎿 🏂 🏋️ 🚴 🏊 🧘 🏆 🥇 🥈 🥉 🏅 🎖️ 🎫 🎟️ 🎭 🎨 "
     "🎬 🎤 🎧 🎼 🎹 🥁 🎷 🎺 🎸 🎻 🎲 🎯 🎳 🎮 🧩 🎉 🎊 🎈 🎁"),
    ("✈️", "Viaggi e luoghi",
     "🚗 🚕 🚙 🚌 🏎️ 🚓 🚑 🚒 🚚 🚜 🛵 🏍️ 🚲 🛴 🚨 🚆 🚇 🚊 🚀 🛸 ✈️ 🛫 🛬 🚁 ⛵ 🚤 🛳️ ⚓ 🗺️ 🏖️ "
     "🏝️ 🏔️ ⛰️ 🌋 🏕️ 🏠 🏡 🏢 🏥 🏦 🏨 🏪 🏫 🏛️ ⛪ 🕌 🏰 🗽 🗼 🎡 🎢 🌃 🌆 🌅 🌄 🌉"),
    ("💡", "Oggetti",
     "⌚ 📱 💻 ⌨️ 🖥️ 🖨️ 🖱️ 💾 💿 📷 📸 📹 🎥 📞 ☎️ 📺 📻 🔋 🔌 💡 🔦 🕯️ 💸 💵 💶 💳 ⚖️ 🔧 🔨 ⚙️ "
     "🔒 🔓 🔑 🗝️ 🛡️ 🔐 📦 📫 📬 ✉️ 📧 📝 ✏️ 🖊️ 📌 📎 📏 📐 ✂️ 🗂️ 📁 📂 📅 📆 🗓️ 📊 📈 📉 📚 📖 "
     "🔍 🔎 🧭 ⏰ ⏳ 🎓 💊 🩺 🧳 🛒"),
    ("✅", "Simboli",
     "✅ ☑️ ✔️ ❌ ❎ ➕ ➖ ✖️ ➗ ❓ ❔ ❗ ❕ ‼️ ⁉️ 💯 🔴 🟠 🟡 🟢 🔵 🟣 ⚫ ⚪ 🟥 🟧 🟨 🟩 🟦 🟪 "
     "⬛ ⬜ 🔶 🔷 ⭕ 🚫 ⛔ ⚠️ 🚸 ♻️ 🔰 💤 💬 💭 🗯️ 🔔 🔕 🎵 🎶 ➡️ ⬅️ ⬆️ ⬇️ ↗️ ↘️ ↙️ ↖️ 🔄 🔃 ▶️ "
     "⏸️ ⏹️ ⏺️ ⏭️ ⏮️ 🔀 🔁 🆗 🆕 🆒 🆓 🆙 🔝 🔜 🏁 🚩 🏳️ 🏴 🇮🇹 🇪🇺"),
]


def elenco():
    """[(icona, nome, [emoji…]), …]"""
    return [(i, n, e.split()) for i, n, e in CATEGORIE]


if __name__ == "__main__":
    import json
    print("// SPDX-License-Identifier: AGPL-3.0-or-later")
    print("// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.")
    print("// Generato da pc/chat/nyctalus_chat/emoji.py: non modificarlo a mano.")
    print("const EMOJI = %s;" % json.dumps(elenco(), ensure_ascii=False))
