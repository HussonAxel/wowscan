-- WowScan : exporte les apparences collectées du compte pour l'application WoWScan.
--
-- Le jeu n'expose pas la liste des apparences collectées d'un coup : on parcourt les
-- identifiants de source (ItemModifiedAppearance) un par un, quelques milliers par
-- image pour ne pas figer le jeu, et on garde l'apparence (visualID) de chaque source
-- collectée. Le résultat est écrit dans WowScanDB, que le jeu enregistre sur disque
-- au /reload ou à la déconnexion.

local BATCH = 4000      -- sources lues par image
local GAP = 40000       -- identifiants vides d'affilée avant de considérer la liste finie
local PREFIX = "|cff33ff99WowScan|r: "
local FRENCH = GetLocale() == "frFR"

local function say(fr, en, ...)
    print(PREFIX .. string.format(FRENCH and fr or en, ...))
end

local frame = CreateFrame("Frame")
local scanning = false
local verbose = false
local collected = nil   -- [appearanceID] = true, après un premier parcours
local writeQueued = false

local function save()
    local ids = {}
    for id in pairs(collected) do
        ids[#ids + 1] = id
    end
    table.sort(ids)
    WowScanDB = WowScanDB or {}
    WowScanDB.version = 1
    WowScanDB.scannedAt = time()
    WowScanDB.count = #ids
    WowScanDB.appearances = table.concat(ids, ",")

    local name, realm = UnitFullName("player")
    realm = realm or GetRealmName()
    local _, _, classID = UnitClass("player")
    if name and realm and classID then
        WowScanDB.characters = WowScanDB.characters or {}
        WowScanDB.characters[name .. "-" .. realm] = classID
    end
    return #ids
end

local function startScan()
    if scanning then return end
    scanning = true
    local found = {}
    local id, lastHit = 1, 0
    frame:SetScript("OnUpdate", function(self)
        local stop = id + BATCH
        while id < stop do
            local info = C_TransmogCollection.GetSourceInfo(id)
            if info then
                lastHit = id
                if info.isCollected and info.visualID and info.visualID > 0 then
                    found[info.visualID] = true
                end
            elseif id - lastHit > GAP then
                self:SetScript("OnUpdate", nil)
                scanning = false
                collected = found
                local count = save()
                if verbose then
                    say("%d apparences collectées. Fais /reload pour que l'application les voie.",
                        "%d appearances collected. Type /reload so the app can read them.", count)
                end
                verbose = false
                return
            end
            id = id + 1
        end
    end)
end

-- Une nouvelle apparence : on l'ajoute sans tout reparcourir (regroupé sur 2 s).
local function onSourceAdded(sourceID)
    if not collected then return end
    local info = sourceID and C_TransmogCollection.GetSourceInfo(sourceID)
    if info and info.visualID and info.visualID > 0 then
        collected[info.visualID] = true
    end
    if not writeQueued then
        writeQueued = true
        C_Timer.After(2, function()
            writeQueued = false
            if collected then save() end
        end)
    end
end

frame:RegisterEvent("PLAYER_LOGIN")
frame:RegisterEvent("TRANSMOG_COLLECTION_SOURCE_ADDED")
frame:RegisterEvent("TRANSMOG_COLLECTION_SOURCE_REMOVED")
frame:SetScript("OnEvent", function(_, event, ...)
    if event == "PLAYER_LOGIN" then
        C_Timer.After(5, startScan)
    elseif event == "TRANSMOG_COLLECTION_SOURCE_ADDED" then
        onSourceAdded(...)
    elseif event == "TRANSMOG_COLLECTION_SOURCE_REMOVED" then
        C_Timer.After(2, startScan)
    end
end)

SLASH_WOWSCAN1 = "/wowscan"
SlashCmdList["WOWSCAN"] = function()
    if scanning then
        say("analyse déjà en cours…", "scan already running…")
        return
    end
    say("analyse de la collection…", "scanning your collection…")
    verbose = true
    startScan()
end
