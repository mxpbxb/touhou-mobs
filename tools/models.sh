#!/usr/bin/env bash
# Regenerates the mob models table from the decompiled Java models.
cd ~/horizon-dev/th-ports || exit 1
python3 -I convert_models.py mobs/native/api/src/models_gen.rs \
  src/kaguya/thKaguyaMod/client/model/living/*.java \
  src/jtg/katrix/journeyToGensokyo/client/model/*.java \
  src/spellcard/spellcardplusmod/client/model/living/*.java \
  src/spellcard/spellcardplusmod/client/model/ModelYin_yang_orb_Yukari.java
