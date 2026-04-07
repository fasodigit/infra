#!/usr/bin/env bash
# =============================================================================
# seed-kaya.sh — Seed KAYA (Redis on port 6380) with realistic Burkinabe data
# Poulets platform: annonces, besoins, aliments, stats
# =============================================================================
set -euo pipefail

REDIS="redis-cli -p 6380"

echo "=== KAYA Seed Script ==="
echo "Connecting to KAYA on port 6380..."
$REDIS PING > /dev/null 2>&1 || { echo "ERROR: KAYA not reachable on port 6380"; exit 1; }

# ---------------------------------------------------------------------------
# Helper: timestamp generator (base date + offset in hours)
# ---------------------------------------------------------------------------
BASE_TS=1775545200  # 2026-04-07T07:00:00Z

ts_offset() {
  local offset_hours=$1
  echo $(( BASE_TS + offset_hours * 3600 ))
}

iso_offset() {
  local offset_hours=$1
  date -u -d "@$(( BASE_TS + offset_hours * 3600 ))" +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || \
    python3 -c "from datetime import datetime,timezone,timedelta; print((datetime(2026,4,7,7,0,0,tzinfo=timezone.utc)+timedelta(hours=$offset_hours)).strftime('%Y-%m-%dT%H:%M:%SZ'))"
}

echo ""
echo "--- Flushing existing poulets:* keys ---"
# Delete existing data in the namespace without flushing the entire DB
for key in $($REDIS KEYS "poulets:*" 2>/dev/null); do
  $REDIS DEL "$key" > /dev/null
done
echo "Done."

# =============================================================================
# ANNONCES (50 entries from eleveurs)
# =============================================================================
echo ""
echo "--- Seeding 50 annonces ---"

# Arrays of realistic Burkinabe data
ELEVEUR_NOMS=(
  "Ouedraogo Amadou"     "Sawadogo Ibrahim"      "Kabore Moussa"
  "Zongo Fatimata"       "Compaore Hamidou"       "Traore Adama"
  "Ouattara Seydou"      "Simpore Rasmata"        "Tiendrebeogo Paul"
  "Nikiema Boureima"     "Ilboudo Salif"          "Diallo Mariama"
  "Belem Abdoulaye"      "Koanda Amidou"          "Yameogo Francois"
  "Zerbo Aissata"        "Toe Lassina"            "Dabire Celestin"
  "Sanou Bintou"         "Konate Drissa"          "Bandaogo Issa"
  "Lingani Mariam"       "Zoungrana Timothee"     "Tapsoba Alidou"
  "Sorgho Wendyam"       "Rouamba Azeta"          "Kafando Issouf"
  "Sessouma Pascal"      "Ouedraogo Zalissa"      "Sawadogo Raogo"
  "Kabore Rosalie"       "Pare Karim"             "Hema Saidou"
  "Congo Therese"        "Barro Souleymane"       "Sanfo Delphine"
  "Guira Noufou"         "Nacoulma Sylvie"        "Zida Arouna"
  "Bance Harouna"        "Zagre Clementine"       "Millogo Yacouba"
  "Tamboura Djibril"     "Somda Bernadette"       "Nebie Ousmane"
  "Yonli Salamata"       "Ouedraogo Kadidia"      "Zorome Hamado"
  "Thiombiano Edith"     "Nacanabo Roger"
)

RACES=("local" "brahma" "pintade" "poulet_chair" "poule_pondeuse")

CITIES=(
  "Ouagadougou, Secteur 1"    "Ouagadougou, Secteur 5"    "Ouagadougou, Secteur 10"
  "Ouagadougou, Secteur 15"   "Ouagadougou, Secteur 17"   "Ouagadougou, Secteur 22"
  "Ouagadougou, Secteur 28"   "Ouagadougou, Secteur 30"   "Bobo-Dioulasso"
  "Bobo-Dioulasso"            "Koudougou"                  "Ouahigouya"
  "Banfora"                   "Tenkodogo"                  "Kaya"
  "Dedougou"                  "Fada N'Gourma"              "Ziniare"
  "Bobo-Dioulasso"            "Koudougou"
)

REGIONS=(
  "Centre"           "Centre"           "Centre"
  "Centre"           "Centre"           "Centre"
  "Centre"           "Centre"           "Hauts-Bassins"
  "Hauts-Bassins"    "Centre-Ouest"     "Nord"
  "Cascades"         "Centre-Est"       "Centre-Nord"
  "Boucle du Mouhoun" "Est"             "Plateau-Central"
  "Hauts-Bassins"    "Centre-Ouest"
)

PHONES_ELV=(
  "+22670112233" "+22671234567" "+22676543210" "+22670987654" "+22675551234"
  "+22672223344" "+22678889900" "+22670001122" "+22673334455" "+22674445566"
  "+22676667788" "+22679998877" "+22671112233" "+22672345678" "+22673456789"
  "+22674567890" "+22675678901" "+22676789012" "+22677890123" "+22678901234"
  "+22670234567" "+22671345678" "+22672456789" "+22673567890" "+22674678901"
  "+22675789012" "+22676890123" "+22677901234" "+22678012345" "+22679123456"
  "+22670345678" "+22671456789" "+22672567890" "+22673678901" "+22674789012"
  "+22675890123" "+22676901234" "+22677012345" "+22678123456" "+22679234567"
  "+22670456789" "+22671567890" "+22672678901" "+22673789012" "+22674890123"
  "+22675901234" "+22676012345" "+22677123456" "+22678234567" "+22679345678"
  "+22670567890" "+22671678901"
)

GROUPEMENTS=(
  "null"
  "\"Cooperative Wend-Panga\""
  "\"GIE Laafi Elevage\""
  "\"Association Teega Wende\""
  "\"Cooperative Relwendé\""
  "\"GPC Nong-Taaba\""
  "null"
  "null"
  "\"Cooperative Avicole de Bobo\""
  "null"
)

DESCRIPTIONS_ANN=(
  "Poulets locaux eleves en plein air, nourris au grain"
  "Poulets de chair bien engraisses, prets a la vente"
  "Pintades sauvages domestiquees, gout authentique"
  "Poules pondeuses reformees, ideales pour ragout"
  "Poulets brahma de grande taille, elevage familial"
  "Volailles locales vaccinees et suivies par veterinaire"
  "Lot de poulets de chair 45 jours, alimentation controlee"
  "Pintades elevees en semi-liberte, chair ferme"
  "Poulets locaux bio sans antibiotiques"
  "Elevage cooperatif, qualite garantie par le groupement"
)

AVAIL_FROM=(
  "2026-05-01" "2026-05-05" "2026-05-10" "2026-05-12" "2026-05-15"
  "2026-05-18" "2026-05-20" "2026-04-15" "2026-04-20" "2026-04-25"
)
AVAIL_TO=(
  "2026-05-20" "2026-05-25" "2026-05-30" "2026-06-01" "2026-06-05"
  "2026-06-10" "2026-06-15" "2026-05-15" "2026-05-20" "2026-05-25"
)

for i in $(seq 1 50); do
  ID=$(printf "ann-%03d" "$i")
  ELV_ID=$(printf "elv-%03d" "$i")
  ELEVEUR="${ELEVEUR_NOMS[$(( (i - 1) % ${#ELEVEUR_NOMS[@]} ))]}"
  PHONE="${PHONES_ELV[$(( (i - 1) % ${#PHONES_ELV[@]} ))]}"

  RACE_IDX=$(( (i - 1) % ${#RACES[@]} ))
  RACE="${RACES[$RACE_IDX]}"

  CITY_IDX=$(( (i - 1) % ${#CITIES[@]} ))
  CITY="${CITIES[$CITY_IDX]}"
  REGION="${REGIONS[$CITY_IDX]}"

  # Price varies by race
  case "$RACE" in
    "local")          PRICE=$(( 2800 + (i * 37) % 700 )) ;;
    "brahma")         PRICE=$(( 4500 + (i * 41) % 1000 )) ;;
    "pintade")        PRICE=$(( 3500 + (i * 53) % 800 )) ;;
    "poulet_chair")   PRICE=$(( 2500 + (i * 29) % 600 )) ;;
    "poule_pondeuse") PRICE=$(( 3000 + (i * 43) % 900 )) ;;
  esac

  QUANTITY=$(( 20 + (i * 17) % 480 ))
  CUR_WEIGHT_10=$(( 10 + (i * 3) % 30 ))   # tenths of kg
  EST_WEIGHT_10=$(( CUR_WEIGHT_10 + 5 + (i * 2) % 15 ))
  CUR_WEIGHT=$(echo "scale=1; $CUR_WEIGHT_10 / 10" | bc)
  EST_WEIGHT=$(echo "scale=1; $EST_WEIGHT_10 / 10" | bc)

  HALAL=$(( i % 7 == 0 ? 0 : 1 ))  # mostly halal, a few not
  HALAL_STR="true"
  [ "$HALAL" -eq 0 ] && HALAL_STR="false"

  VET=$(( i % 5 == 0 ? 0 : 1 ))   # most are vet certified
  VET_STR="true"
  [ "$VET" -eq 0 ] && VET_STR="false"

  GROUPEMENT="${GROUPEMENTS[$(( (i - 1) % ${#GROUPEMENTS[@]} ))]}"
  DESC="${DESCRIPTIONS_ANN[$(( (i - 1) % ${#DESCRIPTIONS_ANN[@]} ))]}"
  AFROM="${AVAIL_FROM[$(( (i - 1) % ${#AVAIL_FROM[@]} ))]}"
  ATO="${AVAIL_TO[$(( (i - 1) % ${#AVAIL_TO[@]} ))]}"

  CREATED=$(iso_offset "$i")
  SCORE=$(ts_offset "$i")

  # Build JSON
  JSON=$(cat <<ENDJSON
{"id":"${ID}","eleveur":"${ELEVEUR}","eleveur_id":"${ELV_ID}","phone":"${PHONE}","race":"${RACE}","quantity":${QUANTITY},"current_weight":${CUR_WEIGHT},"estimated_weight":${EST_WEIGHT},"price_per_kg":${PRICE},"location":"${CITY}","region":"${REGION}","available_from":"${AFROM}","available_to":"${ATO}","halal":${HALAL_STR},"vet_certified":${VET_STR},"groupement":${GROUPEMENT},"description":"${DESC}","status":"active","created_at":"${CREATED}"}
ENDJSON
)

  $REDIS SET "poulets:annonces:${ID}" "$JSON" > /dev/null
  $REDIS ZADD "poulets:annonces:index" "$SCORE" "$ID" > /dev/null
done
echo "50 annonces seeded."

# =============================================================================
# BESOINS (40 entries from clients)
# =============================================================================
echo ""
echo "--- Seeding 40 besoins ---"

CLIENT_NOMS=(
  "Restaurant Le Sahel"            "Restaurant Chez Tantie"         "Hotel Silmande"
  "Cantine Lycee Zinda"            "Menage Ouedraogo"               "Boucherie Moderne Ouaga"
  "Hotel Laico"                    "Restaurant Le Verdoyant"        "Grillades du Kadiogo"
  "Menage Compaore"                "Revendeur Marche Rood Woko"     "Restaurant Eau Vive"
  "Cantine Ecole Saint-Viateur"    "Hotel Palm Beach"               "Evenements Faso Fete"
  "Menage Sawadogo"                "Revendeur Marche Sankariaare"   "Restaurant Le Paysan"
  "Hotel Splendide"                "Cantine Universite Ouaga 1"     "Menage Kabore"
  "Restaurant Gondwana"            "Revendeur Marche de Bobo"       "Hotel Ran"
  "Evenements Burkina Events"      "Menage Traore"                  "Restaurant Chez Amidou"
  "Cantine MENA"                   "Hotel Relax"                    "Revendeur Marche de Koudougou"
  "Menage Ilboudo"                 "Restaurant La Foret"            "Hotel Amiso"
  "Evenements Mariage Royal"       "Cantine Hopital Yalgado"        "Menage Diallo"
  "Restaurant Maquis Vert"        "Revendeur Ouahigouya"           "Hotel Dafra"
  "Menage Belem"
)

CLIENT_TYPES=(
  "restaurant" "restaurant" "hotel"
  "cantine_scolaire" "menage" "revendeur"
  "hotel" "restaurant" "restaurant"
  "menage" "revendeur" "restaurant"
  "cantine_scolaire" "hotel" "evenement"
  "menage" "revendeur" "restaurant"
  "hotel" "cantine_scolaire" "menage"
  "restaurant" "revendeur" "hotel"
  "evenement" "menage" "restaurant"
  "cantine_scolaire" "hotel" "revendeur"
  "menage" "restaurant" "hotel"
  "evenement" "cantine_scolaire" "menage"
  "restaurant" "revendeur" "hotel"
  "menage"
)

PHONES_CLI=(
  "+22670223344" "+22671334455" "+22676445566" "+22670556677" "+22675667788"
  "+22672778899" "+22678880011" "+22670991122" "+22673002233" "+22674113344"
  "+22676224455" "+22679335566" "+22671446677" "+22672557788" "+22673668899"
  "+22674779900" "+22675881011" "+22676992122" "+22677003233" "+22678114344"
  "+22670225566" "+22671336677" "+22672447788" "+22673558899" "+22674660011"
  "+22675771122" "+22676882233" "+22677993344" "+22678004455" "+22679115566"
  "+22670336677" "+22671447788" "+22672558899" "+22673660011" "+22674771122"
  "+22675882233" "+22676993344" "+22677004455" "+22678115566" "+22679226677"
)

FREQUENCIES=("hebdomadaire" "bimensuel" "mensuel" "ponctuel" "quotidien")

NOTES_BES=(
  "Livraison chaque vendredi avant 8h"
  "Poulets vivants de preference"
  "Abattage et plumage inclus svp"
  "Livraison le matin uniquement"
  "Commande reguliere, prix negociable"
  "Besoin urgent pour evenement"
  "Preferons des poulets de plus de 2kg"
  "Livraison a domicile exigee"
  "Besoin de facture pour comptabilite"
  "Achat en gros, remise attendue"
)

DELIVERY_DATES=(
  "2026-05-05" "2026-05-10" "2026-05-15" "2026-05-20" "2026-05-25"
  "2026-06-01" "2026-06-05" "2026-04-20" "2026-04-25" "2026-04-30"
)

for i in $(seq 1 40); do
  ID=$(printf "bes-%03d" "$i")
  CLI_ID=$(printf "cli-%03d" "$i")
  CLIENT="${CLIENT_NOMS[$(( (i - 1) % ${#CLIENT_NOMS[@]} ))]}"
  PHONE="${PHONES_CLI[$(( (i - 1) % ${#PHONES_CLI[@]} ))]}"
  TYPE="${CLIENT_TYPES[$(( (i - 1) % ${#CLIENT_TYPES[@]} ))]}"

  RACE_IDX=$(( (i - 1) % ${#RACES[@]} ))
  RACE="${RACES[$RACE_IDX]}"

  CITY_IDX=$(( (i - 1) % ${#CITIES[@]} ))
  CITY="${CITIES[$CITY_IDX]}"
  REGION="${REGIONS[$CITY_IDX]}"

  QUANTITY=$(( 10 + (i * 13) % 190 ))
  MIN_WEIGHT_10=$(( 15 + (i * 3) % 20 ))
  MIN_WEIGHT=$(echo "scale=1; $MIN_WEIGHT_10 / 10" | bc)
  MAX_BUDGET=$(( 3000 + (i * 47) % 2500 ))

  FREQ="${FREQUENCIES[$(( (i - 1) % ${#FREQUENCIES[@]} ))]}"
  NOTE="${NOTES_BES[$(( (i - 1) % ${#NOTES_BES[@]} ))]}"
  DDATE="${DELIVERY_DATES[$(( (i - 1) % ${#DELIVERY_DATES[@]} ))]}"

  HALAL_REQ=$(( i % 6 == 0 ? 0 : 1 ))
  HALAL_STR="true"
  [ "$HALAL_REQ" -eq 0 ] && HALAL_STR="false"

  VET_REQ=$(( i % 4 == 0 ? 0 : 1 ))
  VET_STR="true"
  [ "$VET_REQ" -eq 0 ] && VET_STR="false"

  CREATED=$(iso_offset "$(( i + 50 ))")
  SCORE=$(ts_offset "$(( i + 50 ))")

  JSON=$(cat <<ENDJSON
{"id":"${ID}","client":"${CLIENT}","client_id":"${CLI_ID}","type":"${TYPE}","race":"${RACE}","quantity":${QUANTITY},"min_weight":${MIN_WEIGHT},"max_budget_per_kg":${MAX_BUDGET},"delivery_date":"${DDATE}","frequency":"${FREQ}","location":"${CITY}","region":"${REGION}","halal_required":${HALAL_STR},"vet_required":${VET_STR},"notes":"${NOTE}","status":"active","created_at":"${CREATED}"}
ENDJSON
)

  $REDIS SET "poulets:besoins:${ID}" "$JSON" > /dev/null
  $REDIS ZADD "poulets:besoins:index" "$SCORE" "$ID" > /dev/null
done
echo "40 besoins seeded."

# =============================================================================
# ALIMENTS (20 entries from producteurs)
# =============================================================================
echo ""
echo "--- Seeding 20 offres aliments ---"

PROD_NOMS=(
  "SOFAB Aliments"                 "Faso Provende"              "SN-CITEC Aliments"
  "Aliments Sahel Plus"            "Provenderie du Centre"      "CAPA Bobo"
  "Avicole Services BF"            "Nutri-Volaille Ouaga"       "Agro-Feed Burkina"
  "Provende Express"               "Bio-Aliments Faso"          "Sahel Nutrition Animale"
  "COPAK Aliments"                 "Provenderie Moderne Ouaga"  "Feed Master BF"
  "SOPROFA"                        "Aliments Premium Volaille"  "Savane Feed"
  "Elevage Solutions"              "ProGrain BF"
)

PRODUCTS=(
  "Formule Demarrage"      "Formule Croissance"     "Formule Finition"
  "Concentre Ponte"        "Premix Vitamines"       "Complement Mineral"
)

PRODUCT_DESCS=(
  "Aliment complet pour poussins 0-4 semaines, 22% proteines"
  "Aliment complet pour poulets de chair 4-8 semaines"
  "Aliment de finition pour poulets de chair 8-12 semaines"
  "Concentre special poules pondeuses, enrichi en calcium"
  "Premix vitaminique pour supplementation quotidienne"
  "Complement mineral pour renforcer la croissance osseuse"
)

TARGET_RACES=("poulet_chair" "poulet_chair" "poulet_chair" "poule_pondeuse" "local" "local")
TARGET_GAINS=(
  "0.3 kg/semaine" "0.5 kg/semaine" "0.4 kg/semaine"
  "amelioration ponte 15%" "soutien immunitaire" "0.2 kg/semaine"
)

ALI_LOCATIONS=(
  "Ouagadougou, Zone Industrielle"    "Ouagadougou, Kossodo"
  "Bobo-Dioulasso, Zone Industrielle" "Koudougou"
  "Ouagadougou, Secteur 29"           "Bobo-Dioulasso"
  "Ouahigouya"                         "Ouagadougou, Secteur 15"
  "Banfora"                            "Ouagadougou, Gounghin"
)
ALI_REGIONS=(
  "Centre"           "Centre"
  "Hauts-Bassins"    "Centre-Ouest"
  "Centre"           "Hauts-Bassins"
  "Nord"             "Centre"
  "Cascades"         "Centre"
)

PHONES_ALI=(
  "+22625301122" "+22625402233" "+22620503344" "+22625604455" "+22625705566"
  "+22620806677" "+22624907788" "+22625008899" "+22620109900" "+22625200011"
  "+22625311122" "+22625422233" "+22620533344" "+22625644455" "+22625755566"
  "+22620866677" "+22624977788" "+22625088899" "+22620199900" "+22625200022"
)

for i in $(seq 1 20); do
  ID=$(printf "ali-%03d" "$i")
  PROD_ID=$(printf "prod-%03d" "$i")
  PRODUCTEUR="${PROD_NOMS[$(( (i - 1) % ${#PROD_NOMS[@]} ))]}"
  PHONE="${PHONES_ALI[$(( (i - 1) % ${#PHONES_ALI[@]} ))]}"

  PROD_IDX=$(( (i - 1) % ${#PRODUCTS[@]} ))
  PRODUCT="${PRODUCTS[$PROD_IDX]}"
  PDESC="${PRODUCT_DESCS[$PROD_IDX]}"
  TRACE="${TARGET_RACES[$PROD_IDX]}"
  TGAIN="${TARGET_GAINS[$PROD_IDX]}"

  LOC_IDX=$(( (i - 1) % ${#ALI_LOCATIONS[@]} ))
  LOCATION="${ALI_LOCATIONS[$LOC_IDX]}"
  REGION="${ALI_REGIONS[$LOC_IDX]}"

  # Price per sac: 8000-18000 depending on product
  case "$PRODUCT" in
    "Formule Demarrage")   PRICE_SAC=$(( 12000 + (i * 31) % 3000 )) ;;
    "Formule Croissance")  PRICE_SAC=$(( 12500 + (i * 37) % 3000 )) ;;
    "Formule Finition")    PRICE_SAC=$(( 11000 + (i * 29) % 2500 )) ;;
    "Concentre Ponte")     PRICE_SAC=$(( 15000 + (i * 43) % 3000 )) ;;
    "Premix Vitamines")    PRICE_SAC=$(( 8000 + (i * 53) % 4000 )) ;;
    "Complement Mineral")  PRICE_SAC=$(( 9000 + (i * 47) % 3500 )) ;;
  esac

  SAC_WEIGHT=$(( 25 + (i * 5) % 3 * 25 ))  # 25 or 50 kg
  [ $SAC_WEIGHT -gt 50 ] && SAC_WEIGHT=50
  STOCK=$(( 50 + (i * 19) % 450 ))

  CREATED=$(iso_offset "$(( i + 90 ))")
  SCORE=$(ts_offset "$(( i + 90 ))")

  JSON=$(cat <<ENDJSON
{"id":"${ID}","producteur":"${PRODUCTEUR}","producteur_id":"${PROD_ID}","phone":"${PHONE}","product":"${PRODUCT}","description":"${PDESC}","price_per_sac":${PRICE_SAC},"sac_weight_kg":${SAC_WEIGHT},"stock_sacs":${STOCK},"location":"${LOCATION}","region":"${REGION}","target_race":"${TRACE}","target_weight_gain":"${TGAIN}","status":"active","created_at":"${CREATED}"}
ENDJSON
)

  $REDIS SET "poulets:aliments:${ID}" "$JSON" > /dev/null
  $REDIS ZADD "poulets:aliments:index" "$SCORE" "$ID" > /dev/null
done
echo "20 offres aliments seeded."

# =============================================================================
# POUSSINS (15 entries from couvoirs/hatcheries)
# =============================================================================
echo ""
echo "--- Seeding 15 poussins ---"

COUVOIR_NOMS=(
  "Couvoir National de Ouaga"
  "Couvoir Moderne de Bobo"
  "Aviculture du Sahel"
  "SOFAB Poussins"
  "Couvoir Faso Koko"
  "Poussins du Houet"
  "Couvoir de la Comoe"
  "Koudougou Aviculture"
  "Couvoir Excellence BF"
  "Poussins du Centre"
  "Sahel Couvoir"
  "Couvoir Wend-Panga"
  "Aviculture Moderne Ouaga"
  "Couvoir du Kadiogo"
  "Poussins Premium Faso"
)

COUVOIR_IDS=(
  "couv-001" "couv-002" "couv-003" "couv-004" "couv-005"
  "couv-006" "couv-007" "couv-008" "couv-009" "couv-010"
  "couv-011" "couv-012" "couv-013" "couv-014" "couv-015"
)

POU_RACES=("local" "brahma" "pintade" "poulet_chair" "poule_pondeuse")
POU_AGES=(1 7 14 21)

POU_LOCATIONS=(
  "Ouagadougou, Zone Industrielle"   "Bobo-Dioulasso, Zone Industrielle"
  "Dori"                              "Ouagadougou, Kossodo"
  "Ouagadougou, Secteur 15"          "Bobo-Dioulasso"
  "Banfora"                           "Koudougou"
  "Ouagadougou, Secteur 29"          "Ouagadougou, Secteur 10"
  "Djibo"                             "Ouagadougou, Secteur 22"
  "Ouagadougou, Gounghin"            "Ouagadougou, Secteur 5"
  "Ouagadougou, Secteur 28"
)

POU_REGIONS=(
  "Centre"          "Hauts-Bassins"
  "Sahel"           "Centre"
  "Centre"          "Hauts-Bassins"
  "Cascades"        "Centre-Ouest"
  "Centre"          "Centre"
  "Sahel"           "Centre"
  "Centre"          "Centre"
  "Centre"
)

POU_AVAIL_FROM=(
  "2026-04-15" "2026-04-20" "2026-04-25" "2026-05-01" "2026-05-05"
  "2026-04-18" "2026-04-22" "2026-04-28" "2026-05-03" "2026-05-08"
  "2026-04-16" "2026-04-21" "2026-04-26" "2026-05-02" "2026-05-06"
)

for i in $(seq 1 15); do
  ID=$(printf "pou-%03d" "$i")
  COUVOIR="${COUVOIR_NOMS[$(( (i - 1) % ${#COUVOIR_NOMS[@]} ))]}"
  COUVOIR_ID="${COUVOIR_IDS[$(( (i - 1) % ${#COUVOIR_IDS[@]} ))]}"

  RACE_IDX=$(( (i - 1) % ${#POU_RACES[@]} ))
  RACE="${POU_RACES[$RACE_IDX]}"

  AGE_IDX=$(( (i - 1) % ${#POU_AGES[@]} ))
  AGE="${POU_AGES[$AGE_IDX]}"

  LOC_IDX=$(( (i - 1) % ${#POU_LOCATIONS[@]} ))
  LOCATION="${POU_LOCATIONS[$LOC_IDX]}"
  REGION="${POU_REGIONS[$LOC_IDX]}"

  AVAIL="${POU_AVAIL_FROM[$(( (i - 1) % ${#POU_AVAIL_FROM[@]} ))]}"

  # Price varies by race and age
  case "$RACE" in
    "local")          BASE_PRICE=500 ;;
    "brahma")         BASE_PRICE=1200 ;;
    "pintade")        BASE_PRICE=800 ;;
    "poulet_chair")   BASE_PRICE=650 ;;
    "poule_pondeuse") BASE_PRICE=900 ;;
  esac
  # Older chicks cost more
  PRICE=$(( BASE_PRICE + AGE * 30 + (i * 23) % 200 ))

  QUANTITY=$(( 100 + (i * 137) % 4900 ))

  # Vaccination: chicks older than 1 day are more likely vaccinated
  if [ "$AGE" -gt 1 ]; then
    VACCINATED="true"
    case "$RACE" in
      "poulet_chair"|"poule_pondeuse") VACC_DETAILS="Marek + Newcastle HB1 + Gumboro" ;;
      "pintade")                        VACC_DETAILS="Newcastle HB1" ;;
      *)                                VACC_DETAILS="Marek + Newcastle HB1" ;;
    esac
  else
    if [ $(( i % 3 )) -eq 0 ]; then
      VACCINATED="false"
      VACC_DETAILS=""
    else
      VACCINATED="true"
      VACC_DETAILS="Marek"
    fi
  fi

  CREATED=$(iso_offset "$(( i + 110 ))")
  SCORE=$(ts_offset "$(( i + 110 ))")

  if [ -n "$VACC_DETAILS" ]; then
    JSON=$(cat <<ENDJSON
{"id":"${ID}","producteur":"${COUVOIR}","producteur_id":"${COUVOIR_ID}","race":"${RACE}","age_jours":${AGE},"quantity":${QUANTITY},"price_unit":${PRICE},"vaccinated":${VACCINATED},"vaccination_details":"${VACC_DETAILS}","location":"${LOCATION}","region":"${REGION}","available_from":"${AVAIL}","status":"active","created_at":"${CREATED}"}
ENDJSON
)
  else
    JSON=$(cat <<ENDJSON
{"id":"${ID}","producteur":"${COUVOIR}","producteur_id":"${COUVOIR_ID}","race":"${RACE}","age_jours":${AGE},"quantity":${QUANTITY},"price_unit":${PRICE},"vaccinated":${VACCINATED},"location":"${LOCATION}","region":"${REGION}","available_from":"${AVAIL}","status":"active","created_at":"${CREATED}"}
ENDJSON
)
  fi

  $REDIS SET "poulets:poussins:${ID}" "$JSON" > /dev/null
  $REDIS ZADD "poulets:poussins:index" "$SCORE" "$ID" > /dev/null
done
echo "15 poussins seeded."

# =============================================================================
# STATS
# =============================================================================
echo ""
echo "--- Seeding stats ---"

$REDIS SET poulets:stats:total_eleveurs 523
$REDIS SET poulets:stats:total_clients 2147
$REDIS SET poulets:stats:total_transactions 51203
$REDIS SET poulets:stats:total_regions 13
$REDIS SET poulets:stats:live_users 847
$REDIS SET poulets:stats:matchings_actifs 23

echo ""
echo "=== Seed complete ==="
echo ""
echo "Verification:"
echo "  total_eleveurs  = $($REDIS GET poulets:stats:total_eleveurs)"
echo "  total_clients   = $($REDIS GET poulets:stats:total_clients)"
echo "  annonces count  = $($REDIS ZCARD poulets:annonces:index)"
echo "  besoins count   = $($REDIS ZCARD poulets:besoins:index)"
echo "  aliments count  = $($REDIS ZCARD poulets:aliments:index)"
echo "  poussins count  = $($REDIS ZCARD poulets:poussins:index)"
echo "  Sample annonce  = $($REDIS GET poulets:annonces:ann-001 | head -c 120)..."
echo "  Sample poussin  = $($REDIS GET poulets:poussins:pou-001 | head -c 120)..."
echo ""
echo "Done. KAYA seeded successfully."
