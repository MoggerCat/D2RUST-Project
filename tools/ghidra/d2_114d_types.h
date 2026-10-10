/*
 * d2_114d_types.h -- Diablo II LoD 1.14d Game.exe in-memory structs for Ghidra's C parser.
 * Ours: written from specs/ (offsets measured on 1.14d). Spec: specs/sim/units.md,
 * specs/sim/stat-lists.md, specs/sim/path-placement.md, specs/sim/tick.md, specs/sim/rng.md,
 * specs/sim/unit-order.md, specs/sim/intents-events.md, specs/drlg/rooms.md, specs/drlg/levels.md,
 * specs/missiles/missiles.md, specs/monsters/ai.md, specs/monsters/init.md,
 * specs/monsters/umod-callbacks.md, specs/world/objects.md, specs/world/quests.md,
 * specs/world/npc.md, specs/items/inventory.md, specs/items/bitstream.md, specs/items/generation.md,
 * specs/formats/d2s.md, specs/data/fields.tsv, specs/data/tables.tsv (plus the other specs named
 * on each struct line). Inventory +0x04 / +0x24 and the game +0xD0 inline seed were confirmed in
 * the 1.14d disassembly. Applied packed (pack(1)): every gap is explicit padding.
 * Fields with no width in the specs are typed u32. Function pointers are void*.
 */

typedef unsigned char u8;
typedef signed char i8;
typedef unsigned short u16;
typedef short i16;
typedef unsigned int u32;
typedef int i32;

typedef struct D2SeedStrc D2SeedStrc;
typedef struct D2UnitStrc D2UnitStrc;
typedef union D2UnitDataUnion D2UnitDataUnion;
typedef struct D2GameStrc D2GameStrc;
typedef struct D2ActiveRoomStrc D2ActiveRoomStrc;
typedef struct D2DrlgRoomStrc D2DrlgRoomStrc;
typedef struct D2DrlgOrthStrc D2DrlgOrthStrc;
typedef struct D2DrlgCoordListStrc D2DrlgCoordListStrc;
typedef struct D2DrlgCoordsStrc D2DrlgCoordsStrc;
typedef struct D2DrlgActStrc D2DrlgActStrc;
typedef struct D2DrlgStrc D2DrlgStrc;
typedef struct D2DrlgLevelStrc D2DrlgLevelStrc;
typedef struct D2DrlgWarpStrc D2DrlgWarpStrc;
typedef struct D2StaticPathStrc D2StaticPathStrc;
typedef struct D2DynamicPathStrc D2DynamicPathStrc;
typedef struct D2StatStrc D2StatStrc;
typedef struct D2StatsArrayStrc D2StatsArrayStrc;
typedef struct D2ModStatsArrayStrc D2ModStatsArrayStrc;
typedef struct D2StatListStrc D2StatListStrc;
typedef struct D2StatListExStrc D2StatListExStrc;
typedef struct D2UnitEventStrc D2UnitEventStrc;
typedef struct D2EventTimerStrc D2EventTimerStrc;
typedef struct D2EventTimerQueueStrc D2EventTimerQueueStrc;
typedef struct D2MonsterDataStrc D2MonsterDataStrc;
typedef struct D2AiControlStrc D2AiControlStrc;
typedef struct D2AiTableStrc D2AiTableStrc;
typedef struct D2AiTickParamStrc D2AiTickParamStrc;
typedef struct D2PlayerDataStrc D2PlayerDataStrc;
typedef struct D2ItemDataStrc D2ItemDataStrc;
typedef struct D2InventoryStrc D2InventoryStrc;
typedef struct D2InventoryGridStrc D2InventoryGridStrc;
typedef struct D2ObjectDataStrc D2ObjectDataStrc;
typedef struct D2ObjectControlStrc D2ObjectControlStrc;
typedef struct D2ObjectRegionStrc D2ObjectRegionStrc;
typedef struct D2MissileDataStrc D2MissileDataStrc;
typedef struct D2MissileStrc D2MissileStrc;
typedef struct D2QuestControlStrc D2QuestControlStrc;
typedef struct D2NpcControlStrc D2NpcControlStrc;
typedef struct D2ArenaStrc D2ArenaStrc;
typedef struct D2PartyControlStrc D2PartyControlStrc;
typedef struct D2MonStatsTxt D2MonStatsTxt;
typedef struct D2MonStats2Txt D2MonStats2Txt;
typedef struct D2SkillsTxt D2SkillsTxt;
typedef struct D2MissilesTxt D2MissilesTxt;
typedef struct D2ItemsTxt D2ItemsTxt;

/* ---- small records ---- */

struct D2SeedStrc { // size 0x08, spec: specs/sim/rng.md §1
    u32 nLowSeed; // +0x00 lo
    u32 nHighSeed; // +0x04 hi
};

struct D2StatStrc { // size 0x08, spec: specs/sim/stat-lists.md §1 (array entry)
    u16 wLayer; // +0x00 layer
    u16 wStatId; // +0x02 stat id (first dword = key)
    i32 nValue; // +0x04 value
};

struct D2StatsArrayStrc { // size 0x08, spec: specs/sim/stat-lists.md §1 (pointer, i16 count, i16 capacity)
    D2StatStrc* pStat; // +0x00 entries
    i16 nStatCount; // +0x04 count
    i16 nCapacity; // +0x06 capacity
};

struct D2ModStatsArrayStrc { // size 0x08, spec: specs/sim/stat-lists.md §1 (mod array, 4-byte keys)
    u32* pStat; // +0x00 keys
    i16 nStatCount; // +0x04 count
    i16 nCapacity; // +0x06 capacity
};

union D2UnitDataUnion { // size 0x04, spec: specs/sim/units.md §2 (+0x14 per-kind data)
    D2PlayerDataStrc* pPlayerData;
    D2MonsterDataStrc* pMonsterData;
    D2ObjectDataStrc* pObjectData;
    D2MissileDataStrc* pMissileData;
    D2ItemDataStrc* pItemData;
};

/* ---- unit ---- */

struct D2UnitStrc { // size 0xF4, spec: specs/sim/units.md §2 (also §4.2, §6.6, world/objects.md §1, sim/intents-events.md, combat/damage.md)
    u32 dwUnitType; // +0x00 unit type
    u32 dwClassId; // +0x04 row of the kind's table
    void* pMemoryPool; // +0x08 memory pool (game +0x1C)
    u32 dwUnitId; // +0x0C GUID
    u32 dwAnimMode; // +0x10 mode
    D2UnitDataUnion pUnitData; // +0x14 per-kind data
    u8 nAct; // +0x18 act
    u8 _pad_19[0x3]; // +0x19
    D2DrlgActStrc* pDrlgAct; // +0x1C act record (game +0xBC + 4*act)
    D2SeedStrc tSeed; // +0x20 unit seed (lo +0x20, hi +0x24)
    u32 dwInitSeed; // +0x28 init seed
    D2DynamicPathStrc* pDynamicPath; // +0x2C path (static path D2StaticPathStrc for objects/items/tiles)
    void* pAnimSeq; // +0x30 sequence record (null: plain animation)
    u32 dwSeqFrameCount; // +0x34 sequence frame count
    i32 nSeqFrame; // +0x38 sequence position; >> 8 = frame event index
    u32 dwSeqSpeed; // +0x3C sequence speed
    u32 dwSeqMode; // +0x40 sequence frame mode
    u32 dwFrameNo; // +0x44 current frame, 8.8
    u32 dwFrameCount; // +0x48 frame count, 8.8
    i16 nAnimSpeed; // +0x4C animation speed (1/256 frame per tick)
    u8 nActionFrame; // +0x4E action frame (last event byte 1-4)
    u8 _pad_4F[0x1]; // +0x4F
    void* pAnimData; // +0x50 AnimData record
    void* pGfxInfo; // +0x54 client: graphics record
    u8 _pad_58[0x4]; // +0x58
    D2StatListExStrc* pStatListEx; // +0x5C stat list (extended)
    D2InventoryStrc* pInventory; // +0x60 inventory
    u32 dwInteractGUID; // +0x64 interact info GUID
    u32 dwInteractType; // +0x68 interact info type
    u8 nInteract; // +0x6C interact info active
    u8 _pad_6D[0x1]; // +0x6D
    u16 wUpdateEvent; // +0x6E unit sound event
    D2UnitStrc* pUpdateTarget; // +0x70 sound event target
    void* pQuestChain; // +0x74 quest chain
    u32 dwSparkOrSounds; // +0x78 object spark byte (server); client: sound request list head
    void* pTimerArg; // +0x7C object timer argument
    D2GameStrc* pGame; // +0x80 game
    u8 _pad_84[0xC]; // +0x84
    D2UnitEventStrc* pUnitEventList; // +0x90 unit event records (combat hooks)
    u32 dwOwnerType; // +0x94 source-unit link: owner type (valid with flag2 0x400)
    u32 dwOwnerGUID; // +0x98 source-unit link: owner GUID
    u8 _pad_9C[0x8]; // +0x9C
    void* pHoverText; // +0xA4 hover text
    void* pSkills; // +0xA8 skill list
    void* pCombat; // +0xAC combat list
    u32 dwHitClass; // +0xB0 hit class of the last hit
    u8 _pad_B4[0x4]; // +0xB4
    u32 dwDropCode; // +0xB8 drop item code (0 = none)
    u8 _pad_BC[0x8]; // +0xBC
    u32 dwFlags; // +0xC4 flags
    u32 dwFlagEx; // +0xC8 flags 2
    u8 _pad_CC[0x4]; // +0xCC
    u32 dwNodeIndex; // +0xD0 target-node list slot (11 = none)
    u32 dwLastOperateTick; // +0xD4 last host tick of a door/gate operation
    void* pClientQueue; // +0xD8 client: queue record
    D2EventTimerStrc* pTimerList; // +0xDC timer list head
    D2UnitStrc* pChangeNextUnit; // +0xE0 update queue link
    D2UnitStrc* pListNext; // +0xE4 hash list next
    D2UnitStrc* pRoomNext; // +0xE8 room unit list next
    void* pPendingEventFirst; // +0xEC pending event records head
    void* pPendingEventLast; // +0xF0 pending event records tail
};

struct D2UnitEventStrc { // size 0x20, spec: specs/sim/units.md §6.6
    u8 nEvent; // +0x00 events.txt index
    u8 _pad_01[0x1]; // +0x01
    u16 wFlags; // +0x02 1 running, 2 remove pending
    u32 dwOwnerKind; // +0x04 owner kind
    u32 dwKey; // +0x08 key
    u32 dwValue0; // +0x0C value 0
    u32 dwValue1; // +0x10 value 1
    void* pfnEvent; // +0x14 function
    D2UnitEventStrc* pPrev; // +0x18 prev
    D2UnitEventStrc* pNext; // +0x1C next
};

/* ---- paths ---- */

struct D2StaticPathStrc { // size 0x20, spec: specs/sim/path-placement.md §2.2
    D2ActiveRoomStrc* pRoom; // +0x00 room
    u32 dwClientX; // +0x04 client x
    u32 dwClientY; // +0x08 client y
    u32 dwSubtileX; // +0x0C sub-tile x
    u32 dwSubtileY; // +0x10 sub-tile y
    u8 _pad_14[0x8]; // +0x14
    u8 nDirection; // +0x1C direction
    u8 bRoomChanged; // +0x1D room-changed flag
    u8 _pad_1E[0x2]; // +0x1E
};

struct D2DynamicPathStrc { // size 0x200, spec: specs/sim/path-placement.md §2.3
    u32 dwPrecisionX; // +0x00 precise x, 16.16 (u16 +0x02 = sub-tile x)
    u32 dwPrecisionY; // +0x04 precise y, 16.16 (u16 +0x06 = sub-tile y)
    u32 dwClientX; // +0x08 client x
    u32 dwClientY; // +0x0C client y
    u16 wTargetX; // +0x10 target x
    u16 wTargetY; // +0x12 target y
    u16 wPrevTargetX; // +0x14 previous target x
    u16 wPrevTargetY; // +0x16 previous target y
    u16 wFinalTargetX; // +0x18 final target x
    u16 wFinalTargetY; // +0x1A final target y
    D2ActiveRoomStrc* pRoom; // +0x1C room
    D2ActiveRoomStrc* pPreviousRoom; // +0x20 previous room
    u32 dwCurrentPointIdx; // +0x24 current point index
    u32 dwPointCount; // +0x28 point count
    u8 _pad_2C[0x4]; // +0x2C
    D2UnitStrc* pUnit; // +0x30 owner unit
    u32 dwFlags; // +0x34 flags
    u32 dwVelocityReset; // +0x38 15 on velocity change, 0 on new path
    u32 dwPathType; // +0x3C path type
    u32 dwPrevPathType; // +0x40 previous path type
    u32 dwUnitSize; // +0x44 unit size
    u32 dwCollisionPattern; // +0x48 collision pattern
    u32 dwFootprintMask; // +0x4C footprint mask
    u32 dwMoveTestMask; // +0x50 move-test mask
    u16 wCollidedMask; // +0x54 collided-with mask
    u8 _pad_56[0x2]; // +0x56
    D2UnitStrc* pTargetUnit; // +0x58 target unit
    u32 dwTargetType; // +0x5C target type
    u32 dwTargetGUID; // +0x60 target GUID
    u8 nDirection; // +0x64 direction
    u8 nNewDirection; // +0x65 new direction
    u8 nTurnStep; // +0x66 turn step
    u8 _pad_67[0x1]; // +0x67
    u8 nTargetLead; // +0x68 target lead
    u8 _pad_69[0x1]; // +0x69
    i32 nDirVectorX; // +0x6A direction vector x (length 4096)
    i32 nDirVectorY; // +0x6E direction vector y
    i32 nVelocityX; // +0x72 velocity vector x, 16.16 per tick
    i32 nVelocityY; // +0x76 velocity vector y
    u8 _pad_7A[0x2]; // +0x7A
    u32 dwVelocity; // +0x7C velocity
    u32 dwSavedVelocity; // +0x80 saved velocity
    u32 dwMaxVelocity; // +0x84 max velocity
    u32 dwAcceleration; // +0x88 acceleration
    u32 dwAccelCounter; // +0x8C acceleration counter
    u8 nDistBudget; // +0x90 distance budget
    u8 nMaxPathDist; // +0x91 max path distance
    u8 nIdaStartScore; // +0x92 IDA* start score
    u8 nStopDist; // +0x93 stop distance
    u8 nRepathBudget; // +0x94 monster re-path budget
    u8 _pad_95[0x3]; // +0x95
    u32 dwDirOffset; // +0x98 direction offset of the path type
    u16 wPathPoints[156]; // +0x9C 78 x {u16 x, u16 y}
    u32 dwSavedStepCount; // +0x1D4 saved-step count
    u16 wSavedSteps[20]; // +0x1D8 10 x {u16 x, u16 y}
};

/* ---- stat lists ---- */

struct D2StatListStrc { // size 0x3C, spec: specs/sim/stat-lists.md §1
    void* pMemoryPool; // +0x00 memory pool
    D2UnitStrc* pUnit; // +0x04 attached unit
    u32 dwOwnerType; // +0x08 owner type
    u32 dwOwnerGUID; // +0x0C owner GUID
    u32 dwFlags; // +0x10 flags (§2)
    u32 dwStateNo; // +0x14 state id
    i32 nExpireFrame; // +0x18 expire frame
    u32 dwSkillNo; // +0x1C skill id
    u32 dwSLvl; // +0x20 skill level
    D2StatsArrayStrc tStats; // +0x24 base array (count +0x28, capacity +0x2A)
    D2StatListStrc* pPrevLink; // +0x2C prev sibling
    D2StatListStrc* pNextLink; // +0x30 next sibling
    D2StatListStrc* pParent; // +0x34 parent list
    void* pfnRemoveCallback; // +0x38 remove callback
};

struct D2StatListExStrc { // size 0x64, spec: specs/sim/stat-lists.md §1 (extended list)
    void* pMemoryPool; // +0x00 memory pool
    D2UnitStrc* pUnit; // +0x04 attached unit
    u32 dwOwnerType; // +0x08 owner type
    u32 dwOwnerGUID; // +0x0C owner GUID
    u32 dwFlags; // +0x10 flags (§2)
    u32 dwStateNo; // +0x14 state id
    i32 nExpireFrame; // +0x18 expire frame
    u32 dwSkillNo; // +0x1C skill id
    u32 dwSLvl; // +0x20 skill level
    D2StatsArrayStrc tStats; // +0x24 base array (unit base stats)
    D2StatListStrc* pPrevLink; // +0x2C prev sibling
    D2StatListStrc* pNextLink; // +0x30 next sibling
    D2StatListStrc* pParent; // +0x34 parent list
    void* pfnRemoveCallback; // +0x38 remove callback
    D2StatListStrc* pLastActive; // +0x3C head of the active child chain
    D2StatListStrc* pLastParked; // +0x40 head of the parked child chain
    D2UnitStrc* pOwner; // +0x44 owner unit
    D2StatsArrayStrc tFullStats; // +0x48 full array (totals)
    D2ModStatsArrayStrc tModStats; // +0x50 mod array (count +0x54, capacity +0x56)
    u32* pStatFlags; // +0x58 state bits, 2 x W u32
    void* pfnCallback; // +0x5C value-change callback
    D2GameStrc* pGame; // +0x60 game
};

/* ---- DRLG ---- */

struct D2DrlgOrthStrc { // size 0x18, spec: specs/drlg/rooms.md §1 (link list node)
    void* pTarget; // +0x00 target room, or the level for a cross-level link
    u32 dwDirection; // +0x04 direction 0..3
    u32 dwExtra; // +0x08 extra
    u32 dwInit; // +0x0C init flag
    void* pBox; // +0x10 target box (x, y, w, h)
    D2DrlgOrthStrc* pNext; // +0x14 next
};

struct D2DrlgCoordsStrc { // size 0x30, spec: specs/drlg/levels.md §11.1 (coordinate record)
    i32 nBoxX0; // +0x00 box x0 (level tiles)
    i32 nBoxY0; // +0x04 box y0
    i32 nBoxX1; // +0x08 box x1 (exclusive)
    i32 nBoxY1; // +0x0C box y1 (exclusive)
    i32 nClipX0; // +0x10 clipped box x0
    i32 nClipY0; // +0x14 clipped box y0
    i32 nClipX1; // +0x18 clipped box x1
    i32 nClipY1; // +0x1C clipped box y1
    u32 dwNode; // +0x20 node flag
    u32 dwUnused; // +0x24 not written (0)
    u32 dwIndex; // +0x28 index
    D2DrlgCoordsStrc* pNext; // +0x2C next
};

struct D2DrlgCoordListStrc { // size 0x34, spec: specs/drlg/levels.md §11.1 (logical-room info, DRLG room +0x64)
    u32 dwFlags; // +0x00 1 whole room, 2 built from grids
    u32 dwRecords; // +0x04 records allocated as one array
    u8 tIndexGrid[0x14]; // +0x08 index grid record, flag 2 only (extent to next field)
    u8 tRecordGrid[0x14]; // +0x1C record grid: a coordinate-record pointer per tile (extent to next field)
    D2DrlgCoordsStrc* pFirst; // +0x30 first coordinate record
};

struct D2DrlgRoomStrc { // size 0xEC, spec: specs/drlg/rooms.md §1
    D2DrlgOrthStrc* pOrth; // +0x00 link list
    u32 dwInitSeed; // +0x04 dwInitSeed
    D2DrlgRoomStrc** ppRoomsNear; // +0x08 rooms-near array
    u16 wStatusRefs[4]; // +0x0C status reference counts (statuses 0..3)
    D2SeedStrc tSeed; // +0x14 room seed (lo, hi)
    D2DrlgRoomStrc* pStatusNext; // +0x1C status list next
    u8 _pad_20[0x4]; // +0x20
    D2DrlgRoomStrc* pDrlgRoomNext; // +0x24 next room of the level
    u32 dwFlags; // +0x28 room flags
    u32 nRoomsNear; // +0x2C rooms-near count
    D2ActiveRoomStrc* pRoom; // +0x30 active room (0 = none)
    u32 dwTileX; // +0x34 tile x
    u32 dwTileY; // +0x38 tile y
    u32 dwTileW; // +0x3C tile width
    u32 dwTileH; // +0x40 tile height
    u8 nStatus; // +0x44 status 0..4
    u8 _pad_45[0x3]; // +0x45
    u32 dwRoomType; // +0x48 1 outdoor-grid, 2 preset
    void* pWarpLinks; // +0x4C warp links
    u8 _pad_50[0x4]; // +0x50
    void* pTileGrid; // +0x54 tile grid
    D2DrlgLevelStrc* pLevel; // +0x58 level
    void* pPresetUnits; // +0x5C preset units
    u32 dwOtherFlags; // +0x60 bit 0 was populated
    D2DrlgCoordListStrc* pCoordList; // +0x64 logical-room info
    u8 _pad_68[0x80]; // +0x68
    D2DrlgRoomStrc* pStatusPrev; // +0xE8 status list previous
};

struct D2ActiveRoomStrc { // size 0x80, spec: specs/drlg/rooms.md §1 (also sim/units.md §3, sim/unit-order.md)
    D2ActiveRoomStrc** ppRoomList; // +0x00 adjacency array
    u32 dwClientCapacity; // +0x04 client array capacity
    void* pTileData; // +0x08 tile data
    u32 dwInactiveFrames; // +0x0C inactivity counter
    D2DrlgRoomStrc* pDrlgRoom; // +0x10 DRLG room
    u8 nDeadRingIdx; // +0x14 last-dead GUID ring index
    u8 _pad_15[0x7]; // +0x15
    D2UnitStrc* pUpdateQueue; // +0x1C update queue head
    void* pCollisionGrid; // +0x20 collision grid header
    u32 nRoomsNear; // +0x24 adjacency count
    u8 _pad_28[0x4]; // +0x28
    u32 dwAct; // +0x2C act
    u8 _pad_30[0x4]; // +0x30
    u32 dwFlags; // +0x34 bit 0 populated, 1 units active, 2 no update
    u32 dwLastDeadGUIDs[4]; // +0x38 ring of the last four dead GUIDs
    D2UnitStrc** ppClients; // +0x48 client array
    u32 dwSubtileX; // +0x4C sub-tile x
    u32 dwSubtileY; // +0x50 sub-tile y
    u32 dwSubtileW; // +0x54 sub-tile w
    u32 dwSubtileH; // +0x58 sub-tile h
    u32 dwTileX; // +0x5C tile x
    u32 dwTileY; // +0x60 tile y
    u32 dwTileW; // +0x64 tile w
    u32 dwTileH; // +0x68 tile h
    D2SeedStrc tSeed; // +0x6C active-room seed
    D2UnitStrc* pUnitFirst; // +0x74 first unit
    u32 nClients; // +0x78 client count
    D2ActiveRoomStrc* pRoomNext; // +0x7C next in act list
};

struct D2DrlgActStrc { // size 0x60, spec: specs/drlg/levels.md §1 (act)
    u8 _pad_00[0x4]; // +0x00
    void* pEnvironment; // +0x04 environment
    u32 dwTownLevelId; // +0x08 town level id (server)
    u32 dwInitSeed; // +0x0C init seed
    D2ActiveRoomStrc* pRoomFirst; // +0x10 active-room list head
    u32 dwAct; // +0x14 act no
    u8 _pad_18[0x30]; // +0x18
    D2DrlgStrc* pDrlg; // +0x48 drlg
    void* pfnRoomCallback; // +0x4C room callback
    u32 dwClientFlag; // +0x50 client flag
    u32 dwPendingRoom; // +0x54 pending-room flag
    u8 _pad_58[0x4]; // +0x58
    void* pMemoryPool; // +0x5C memory pool
};

struct D2DrlgWarpStrc { // size 0x48, spec: specs/drlg/levels.md §1 (vis/warp record)
    u32 dwLevelId; // +0x00 level id
    u32 dwVis[8]; // +0x04 vis[8]
    u32 dwWarp[8]; // +0x24 warp[8]
    D2DrlgWarpStrc* pNext; // +0x44 next
};

struct D2DrlgStrc { // size 0x48C, spec: specs/drlg/levels.md §1 (drlg)
    D2SeedStrc tSeed; // +0x00 DRLG seed (lo, hi)
    u32 dwRoomsAllocated; // +0x08 rooms allocated (stat)
    u8 _pad_0C[0x80]; // +0x0C
    u32 dwFlags; // +0x8C bit 0 on client
    D2DrlgWarpStrc* pWarp; // +0x90 vis/warp record list
    u32 dwStaffTombLevel; // +0x94 Act 2 staff tomb level
    u8 nRoomsBuilt; // +0x98 rooms built since last client update
    u8 _pad_99[0x3]; // +0x99
    D2GameStrc* pGame; // +0x9C game
    D2DrlgRoomStrc tStatusRooms[4]; // +0xA0 room status lists 0..3 (dummy head rooms)
    u8 nDifficulty; // +0x450 difficulty
    u8 _pad_451[0x3]; // +0x451
    void* pfnAutomap; // +0x454 automap callback
    u32 dwInitSeedCopy; // +0x458 init seed copy
    u8 _pad_45C[0x10]; // +0x45C
    D2DrlgActStrc* pAct; // +0x46C act
    u32 dwStartSeed; // +0x470 dwStartSeed
    u32 dwJungleLink; // +0x474 Act 3 jungle-link bit
    void* pMemoryPool; // +0x478 memory pool
    D2DrlgLevelStrc* pLevel; // +0x47C level list head
    u8 nAct; // +0x480 act no
    u8 _pad_481[0x3]; // +0x481
    u32 dwBossTombLevel; // +0x484 Act 2 boss tomb level
    void* pfnAutomap2; // +0x488 automap callback 2
};

struct D2DrlgLevelStrc { // size 0x230, spec: specs/drlg/levels.md §1 (level), §11.1
    u32 dwDrlgType; // +0x00 1 maze, 2 preset, 3 outdoor
    u32 dwFlags; // +0x04 0x10 automap reveal
    u32 nRooms; // +0x08 room count
    u32 dwActivity; // +0x0C activity count
    D2DrlgRoomStrc* pFirstRoomEx; // +0x10 first room
    void* pTypeInfo; // +0x14 type info (preset info for type 2)
    u8 _pad_18[0x4]; // +0x18
    u32 dwPosX; // +0x1C position x (tiles)
    u32 dwPosY; // +0x20 position y
    u32 dwSizeX; // +0x24 width
    u32 dwSizeY; // +0x28 height
    u32 dwSpawnTiles[96]; // +0x2C spawn-tile records {x, y, tile index} stride 12 (extent to next field)
    D2DrlgLevelStrc* pNextLevel; // +0x1AC next level
    u8 _pad_1B0[0x4]; // +0x1B0
    D2DrlgStrc* pDrlg; // +0x1B4 drlg
    u32 dwJungleCount; // +0x1B8 Act III jungle clearing count
    void* pJungleBlocks; // +0x1BC jungle block ids
    u32 dwLevelType; // +0x1C0 level type
    D2SeedStrc tSeed; // +0x1C4 level seed (lo, hi)
    u8 _pad_1CC[0x4]; // +0x1CC
    u32 dwLevelId; // +0x1D0 level id
    u32 dwInactiveFrames; // +0x1D4 inactive frames
    u32 nSpawnTiles; // +0x1D8 spawn-tile count
    u32 dwCoordListCounter; // +0x1DC coordinate-list counter
    u32 dwWarpX[9]; // +0x1E0 warp-room centres x
    u32 dwWarpY[9]; // +0x204 warp-room centres y
    u32 nWarps; // +0x228 warp count
    void* pPopulatedRooms; // +0x22C populated-room memory
};

/* ---- per-kind unit data ---- */

struct D2PlayerDataStrc { // size 0x16C, spec: specs/formats/d2s.md, specs/sim/path-placement.md §10, specs/world/npc.md, specs/client/model.md (size not stated: ends at the last known field)
    char szName[16]; // +0x00 player name
    void* pQuestData[3]; // +0x10 quest record per difficulty
    void* pWaypointData[3]; // +0x1C waypoint record per difficulty
    u8 _pad_28[0x4]; // +0x28
    u32 dwUnk2C; // +0x2C set by S->C 0x5F (pdata_2c)
    u8 _pad_30[0x4]; // +0x30
    void* pArenaUnit; // +0x34 arena record
    u8 _pad_38[0xC]; // +0x38
    void* pPetList; // +0x44 pet lists
    u32 dwPortalGUID; // +0x48 own town portal object GUID
    u32 dwBusy; // +0x4C busy
    u32 dwTradeState; // +0x50 trade state
    u8 _pad_54[0x8]; // +0x54
    void* pTrade; // +0x5C pTrade
    void* pNpcIntro[3]; // +0x60 NPC intro record per difficulty
    u32 dwCopyGUID; // +0x6C copy GUID
    u32 dwRightSkill; // +0x70 right skill
    u32 dwLeftSkill; // +0x74 left skill
    u32 dwRightSkillItem; // +0x78 right skill item
    u32 dwLeftSkillItem; // +0x7C left skill item
    u32 dwSwitchRightSkill; // +0x80 swap right skill
    u32 dwSwitchLeftSkill; // +0x84 swap left skill
    u32 dwSwitchRightItem; // +0x88 swap right item
    u32 dwSwitchLeftItem; // +0x8C swap left item
    u32 dwUsedItemGUID; // +0x90 item GUID used by a skill
    u8 _pad_94[0x8]; // +0x94
    void* pClient; // +0x9C client
    u8 nHistoryIdx; // +0xA0 position history next index
    u8 _pad_A1[0x3]; // +0xA1
    u32 dwHistoryTick; // +0xA4 time of the last history write
    u32 dwHistory[40]; // +0xA8 position history 20 x {x, y}
    u32 dwLastPlacedX; // +0x148 last placed point x
    u32 dwLastPlacedY; // +0x14C last placed point y
    u32 dwPendingFlag; // +0x150 pending action flag
    u32 dwPendingCode; // +0x154 pending action code
    u32 dwPendingType; // +0x158 pending action unit type
    u32 dwPendingGUID; // +0x15C pending action GUID
    u32 dwWaypointTick; // +0x160 GetTickCount value
    u8 _pad_164[0x4]; // +0x164
    u32 dwLastMsgFrame; // +0x168 frame of the last gated message
};

struct D2MonsterDataStrc { // size 0x60, spec: specs/monsters/init.md (outputs), specs/monsters/ai.md §3.1, specs/monsters/umod-callbacks.md §3.6, specs/sim/units.md §3
    D2MonStatsTxt* pMonstatsTxt; // +0x00 monstats record
    u8 nComponent[16]; // +0x04 component bytes
    u16 wNameSeed; // +0x14 name seed
    u16 wTypeFlag; // +0x16 type flags
    i32 nLastBurstFrame; // +0x18 frame of the last lightning burst
    u8 nMonUmod[9]; // +0x1C umods, 0-terminated
    u8 _pad_25[0x1]; // +0x25
    u16 wBossHcIdx; // +0x26 superunique row
    D2AiControlStrc* pAiControl; // +0x28 AI control
    void* pAiParam; // +0x2C AI param record
    void* pMonInteract; // +0x30 interaction block
    u32 dwUnk34; // +0x34 cleared with +0x38
    u32 dwUnk38; // +0x38
    u32 dwUnk3C; // +0x3C -1 = none (act5pow)
    u32 dwUnk40; // +0x40 written by S->C 0x98
    u8 _pad_44[0xC]; // +0x44
    void* pVision; // +0x50 room coord list
    u32 dwAiState; // +0x54 AI state
    u32 dwTxtLevelNo; // +0x58 level id
    u32 dwSummonerFlags; // +0x5C bit 0, bit 2 summoner flags
};

struct D2AiControlStrc { // size 0x40, spec: specs/monsters/ai.md §3.1
    u32 nAiSpecialState; // +0x00 special state
    void* pAiParamFn; // +0x04 current AI function
    u16 nAiFlags; // +0x08 AI flags
    u8 _pad_0A[0x2]; // +0x0A
    u32 dwOwnerGUID; // +0x0C leash owner GUID
    u32 dwOwnerType; // +0x10 leash owner type
    u32 dwAiParam[3]; // +0x14 per-AI scratch
    void* pCurrentCmd; // +0x20 command ring current
    void* pLastCmd; // +0x24 command ring last
    D2GameStrc* pGame; // +0x28 nonzero = minion bookkeeping
    u32 dwOwnerGUIDEx; // +0x2C minion owner GUID
    u32 dwOwnerTypeEx; // +0x30 minion owner type
    void* pMinionList; // +0x34 minion GUID list
    void* pMapAi; // +0x38 preset path nodes
    u32 nMinionSpawnClassId; // +0x3C spawner class
};

struct D2AiTableStrc { // size 0x10, spec: specs/monsters/ai.md §3.2
    u32 dwTargetMode; // +0x00 target mode
    void* pfnInit; // +0x04 init function
    void* pfnThink; // +0x08 think function
    void* pfnAlternate; // +0x0C alternate function
};

struct D2AiTickParamStrc { // size 0x24, spec: specs/monsters/ai.md §2.1
    D2AiControlStrc* pAiControl; // +0x00 monster data +0x28
    u32 unk0x04; // +0x04 0
    D2UnitStrc* pTarget; // +0x08 target
    u32 unk0x0C; // +0x0C 0
    u32 unk0x10; // +0x10 0
    u32 nTargetDistance; // +0x14 target distance
    u32 bCombat; // +0x18 target in melee range
    D2MonStatsTxt* pMonstatsTxt; // +0x1C monstats row
    D2MonStats2Txt* pMonstats2Txt; // +0x20 monstats2 row
};

struct D2ItemDataStrc { // size 0x74, spec: specs/items/inventory.md §1.1, specs/items/bitstream.md, specs/items/generation.md, specs/sim/rng.md
    u32 dwQualityNo; // +0x00 quality
    D2SeedStrc tSeed; // +0x04 item seed
    u32 dwOwnerGUID; // +0x0C owner-player GUID (-1 none)
    u32 dwInitSeed; // +0x10 start seed
    u32 dwCommandFlags; // +0x14 command flags
    u32 dwItemFlags; // +0x18 item flags
    u32 dwRealmData[2]; // +0x1C realm data
    u32 dwExpireFrame; // +0x24 ground expiry
    i32 nFileIndex; // +0x28 unique/set/superior index
    u32 dwItemLevel; // +0x2C item level
    u16 wVersion; // +0x30 item format / version
    u16 wRarePrefix; // +0x32 rare prefix
    u16 wRareSuffix; // +0x34 rare suffix
    u16 wAutoAffix; // +0x36 automagic
    u16 wMagicPrefix[3]; // +0x38 magic prefixes
    u16 wMagicSuffix[3]; // +0x3E magic suffixes
    u8 nBodyLoc; // +0x44 body location
    u8 nInvPage; // +0x45 page (0xFF none)
    u8 _pad_46[0x1]; // +0x46
    u8 nStoredPage; // +0x47 stored page
    u8 _pad_48[0x1]; // +0x48
    u8 nInvGfxIdx; // +0x49 gfx variant
    char szPlayerName[16]; // +0x4A ear / personalized name
    u8 _pad_5A[0x2]; // +0x5A
    D2InventoryStrc* pParentInv; // +0x5C owning inventory
    D2UnitStrc* pPrevItem; // +0x60 item list prev
    D2UnitStrc* pNextItem; // +0x64 item list next
    u8 nGridPlus1; // +0x68 grid + 1 (0 = none)
    u8 nNodePos; // +0x69 node kind
    u8 _pad_6A[0x2]; // +0x6A
    D2UnitStrc* pPrevGridItem; // +0x6C grid list prev
    D2UnitStrc* pNextGridItem; // +0x70 grid list next
};

struct D2InventoryGridStrc { // size 0x10, spec: specs/items/inventory.md §1.1 (grid)
    D2UnitStrc* pFirstItem; // +0x00 grid item list first
    D2UnitStrc* pLastItem; // +0x04 grid item list last
    u8 nGridWidth; // +0x08 width
    u8 nGridHeight; // +0x09 height
    u8 _pad_0A[0x2]; // +0x0A
    D2UnitStrc** ppItems; // +0x0C cells, row-major
};

struct D2InventoryStrc { // size 0x40, spec: specs/items/inventory.md §1.1, specs/combat/vitals.md, specs/world/hirelings.md (size, +0x04, +0x24 confirmed in all.asm 0x0063ABD0)
    u32 dwSignature; // +0x00 0x01020304
    void* pMemoryPool; // +0x04 memory pool (owner's)
    D2UnitStrc* pOwner; // +0x08 owner unit
    D2UnitStrc* pFirstItem; // +0x0C item list first
    D2UnitStrc* pLastItem; // +0x10 item list last
    D2InventoryGridStrc* pGrids; // +0x14 grid array
    u32 nGridCount; // +0x18 grid count
    u32 dwLeftItemGUID; // +0x1C GUID of the weapon in use (-1 none)
    D2UnitStrc* pCursorItem; // +0x20 cursor item
    u32 dwOwnerGUID; // +0x24 owner GUID
    u32 dwItemCount; // +0x28 linked items
    void* pFirstNode; // +0x2C update list head
    void* pLastNode; // +0x30 update list tail
    void* pFirstCorpse; // +0x34 corpse list
    u8 _pad_38[0x4]; // +0x38
    u32 nCorpseCount; // +0x3C corpse count
};

struct D2ObjectDataStrc { // size 0x38, spec: specs/world/objects.md §1
    void* pObjectTxt; // +0x00 objects.txt record
    u8 nInteractType; // +0x04 InteractType
    u8 nPortalFlags; // +0x05 portal flags
    u8 _pad_06[0x2]; // +0x06
    void* pShrineTxt; // +0x08 shrines.txt record
    u32 dwOperateGUID; // +0x0C operator GUID + 1
    u8 _pad_10[0x8]; // +0x10
    u32 dwDestX; // +0x18 portal destination x
    u32 dwDestY; // +0x1C portal destination y
    u8 _pad_20[0x18]; // +0x20
};

struct D2MissileDataStrc { // size 0x34, spec: specs/missiles/missiles.md §R1
    u8 _pad_00[0x8]; // +0x00
    i16 nActivateFrame; // +0x08 activate frame
    i16 nSkill; // +0x0A skill
    i16 nLevel; // +0x0C level
    i16 nTotalFrames; // +0x0E total frames
    i16 nCurrentFrame; // +0x10 frames left
    u8 _pad_12[0x2]; // +0x12
    u32 dwFlags; // +0x14 flags
    u32 dwLastCollideType; // +0x18 last-collided unit type
    u32 dwLastCollideGUID; // +0x1C last-collided GUID (-1 none)
    u8 _pad_20[0x8]; // +0x20
    i32 nTargetX; // +0x28 target field x (server-do state)
    i32 nTargetY; // +0x2C target field y
    u8 _pad_30[0x4]; // +0x30
};

struct D2MissileStrc { // size 0x5C, spec: specs/missiles/missiles.md §R2.1
    u32 dwFlags; // +0x00 flags
    D2UnitStrc* pOwner; // +0x04 owner
    D2UnitStrc* pOrigin; // +0x08 origin unit
    D2UnitStrc* pTarget; // +0x0C target unit
    u32 dwMissile; // +0x10 missile class
    i32 nX; // +0x14 x
    i32 nY; // +0x18 y
    i32 nTargetX; // +0x1C target x
    i32 nTargetY; // +0x20 target y
    u32 dwGfxArg; // +0x24 gfx argument
    i32 nVelocity; // +0x28 velocity
    i32 nSkill; // +0x2C skill id
    i32 nSkillLevel; // +0x30 skill level
    i32 nLoops; // +0x34 loops
    u8 _pad_38[0x8]; // +0x38
    i32 nFrame; // +0x40 start frame
    i32 nActivateFrame; // +0x44 activate frames
    i32 nAttBonus; // +0x48 attack bonus
    i32 nRange; // +0x4C range
    i32 nLightRadius; // +0x50 light radius
    void* pfnInit; // +0x54 init callback
    void* pInitArg; // +0x58 init callback argument
};

/* ---- game and game-owned controls ---- */

struct D2EventTimerStrc { // size 0x30, spec: specs/sim/tick.md §5.1 (timer record; size not stated: ends at the last field)
    u8 nEventType; // +0x00 event type
    u8 _pad_01[0x1]; // +0x01
    u16 wFlags; // +0x02 1 executing, 2 free, 4 every-tick, 8 delete after
    i32 nExpireFrame; // +0x04 expire frame (-1 every tick)
    D2UnitStrc* pUnit; // +0x08 unit
    u32 dwUnitGUID; // +0x0C unit GUID
    u32 dwUnitType; // +0x10 unit type (6 none)
    u32 dwEventCustomId; // +0x14 event argument 1
    u32 dwEventCustomParam; // +0x18 event argument 2
    D2EventTimerStrc* pNext; // +0x1C next in bucket / every-tick list
    D2EventTimerStrc* pPrev; // +0x20 previous in bucket / every-tick list
    D2EventTimerStrc* pUnitNext; // +0x24 next in the unit's timer list
    D2EventTimerStrc* pUnitPrev; // +0x28 previous in the unit's timer list
    void* pfnCallback; // +0x2C callback (null = class default)
};

struct D2EventTimerQueueStrc { // size 0xA20, spec: specs/sim/tick.md §5.1 (size not stated: ends at the last field)
    u32 nCurrentBucket; // +0x00 frame % 64
    D2EventTimerStrc* pBucketHead[320]; // +0x04 [class 0..4][bucket 0..63] heads
    D2EventTimerStrc* pBucketTail[320]; // +0x504 [class 0..4][bucket 0..63] tails
    D2EventTimerStrc* pInfinite[5]; // +0xA04 every-tick list per class
    D2EventTimerStrc* pCursor; // +0xA18 iteration cursor
    void* pSlab; // +0xA1C first timer slab
};

struct D2QuestControlStrc { // size 0x24, spec: specs/world/quests.md §2.1
    void* pLastQuest; // +0x00 newest quest record
    u32 bExecuting; // +0x04 1 while the updater runs
    u32 bPickedSet; // +0x08 quest set picked
    void* pQuestFlags; // +0x0C game quest flag record
    void* pTimer; // +0x10 timer list head
    u32 dwTick; // +0x14 updater tick counter
    D2SeedStrc tSeed; // +0x18 quest seed
    u8 nFxByte; // +0x20 FX byte for 0x89
    u8 _pad_21[0x3]; // +0x21
};

struct D2ObjectRegionStrc { // size 0x90, spec: specs/world/objects.md §2 (level region)
    u8 nAct; // +0x00 levels.Act
    u8 _pad_01[0x7]; // +0x01
    u32 dwUnk08; // +0x08 0x7FFFFFFF at creation
    u8 _pad_0C[0x10]; // +0x0C
    u32 dwUnk1C; // +0x1C -1 at creation
    u8 _pad_20[0x70]; // +0x20
};

struct D2ObjectControlStrc { // size 0x1114, spec: specs/world/objects.md §2
    D2SeedStrc tSeed; // +0x00 control seed
    u32 nShrines[8]; // +0x08 shrine count per effectclass
    u32* pShrineLists[8]; // +0x28 shrine row lists per effectclass
    D2ObjectRegionStrc* pRegions[1024]; // +0x48 region per level id
    u8 _pad_1048[0xC8]; // +0x1048 zeroed block
    u32 dwUnk1110; // +0x1110 0 at creation
};

struct D2NpcControlStrc { // size 0x14, spec: specs/world/npc.md §1.1 (size not stated: ends at the last field)
    u32 nRecords; // +0x00 record count
    void* pRecords; // +0x04 record array (0x44 bytes each)
    D2SeedStrc tSeed; // +0x08 NPC-control seed
    u32 nCount; // +0x10 count (same value)
};

struct D2ArenaStrc { // size 0x10, spec: specs/sim/intents-events.md §8.1
    u32 dwUnk00; // +0x00 0
    u32 dwUnk04; // +0x04 0
    u32 dwFlags; // +0x08 arena flags
    u32 dwType; // +0x0C low byte from C->S 0x67
};

struct D2PartyControlStrc { // size 0x08, spec: specs/sim/intents-events.md §8.1, specs/world/quests-act1-rest.md §6
    u16 wUnk00; // +0x00 3 at creation
    u8 _pad_02[0x2]; // +0x02
    void* pFirstParty; // +0x04 first party
};

struct D2GameStrc { // size 0x1DF4, spec: specs/sim/tick.md, specs/sim/unit-order.md §1-§2, specs/sim/rng.md, specs/monsters/init.md, specs/world/objects.md §2, specs/world/quests.md §2.1 (size not stated: ends at the last known field)
    u8 _pad_00[0x1C]; // +0x00
    u8 tMemoryPool[0xC]; // +0x1C memory pool (unit +0x08 holds it; extent to next field)
    u16 wGameNo; // +0x28 game number (S->C 0xB2 u16@0x33)
    char szGameName[16]; // +0x2A game name
    u8 _pad_3A[0x30]; // +0x3A
    u8 nGameType; // +0x6A game type
    u8 nArenaByte; // +0x6B from C->S 0x67 u8@0x13
    u8 _pad_6C[0x1]; // +0x6C
    u8 nDifficulty; // +0x6D difficulty
    u8 _pad_6E[0x2]; // +0x6E
    u32 bExpansion; // +0x70 expansion
    u32 dwLadder; // +0x74 ladder
    u16 wItemFormat; // +0x78 item format
    u8 _pad_7A[0x2]; // +0x7A
    u32 dwInitSeed; // +0x7C init seed (map seed)
    u32 dwObjSeed; // +0x80 object-control seed result
    u32 bFixedSeed; // +0x84 -seed switch flag
    void* pClientList; // +0x88 client list head
    u32 nClients; // +0x8C client count
    u32 dwSpawnedUnits[6]; // +0x90 GUID counters per unit type
    u32 dwGameFrame; // +0xA8 frame
    u32 dwFrameRate; // +0xAC frames per second
    u32 dwFrameCount; // +0xB0 frame count
    u8 _pad_B4[0x4]; // +0xB4
    D2EventTimerQueueStrc* pTimerQueue; // +0xB8 timer queue
    D2DrlgActStrc* pAct[5]; // +0xBC act records
    D2SeedStrc tGameSeed; // +0xD0 game seed (pGameSeed)
    u8 _pad_D8[0x14]; // +0xD8
    u32 dwMonSeed; // +0xEC dwMonSeed
    void* pMonReg[1024]; // +0xF0 monster region per level id
    D2ObjectControlStrc* pObjectControl; // +0x10F0 object control
    D2QuestControlStrc* pQuestControl; // +0x10F4 quest control
    void* pTargetNodes[10]; // +0x10F8 target-node list heads
    D2UnitStrc* pUnitList[640]; // +0x1120 unit hash lists [5 lists][128 buckets]
    D2UnitStrc* pTileList; // +0x1B20 tile unit list
    u32 dwUniqueFlags[128]; // +0x1B24 unique-dropped bits
    D2NpcControlStrc* pNpcControl; // +0x1D24 NPC control
    D2ArenaStrc* pArena; // +0x1D28 arena record
    D2PartyControlStrc* pPartyControl; // +0x1D2C party list
    u32 dwBossFlags[16]; // +0x1D30 boss-spawned bitset (extent to next field)
    u32 dwAiCounters[16]; // +0x1D70 AI counters, + 4*c
    u8 _pad_1DB0[0x4]; // +0x1DB0
    u32 dwAiCounterTotal; // +0x1DB4 counter for c != 0
    u8 _pad_1DB8[0x4]; // +0x1DB8
    u32 dwLoadRatio; // +0x1DBC per-game load ratio
    u8 _pad_1DC0[0x4]; // +0x1DC0
    u32 dwSyncTimer; // +0x1DC4 sync timer
    u32 dwDebugTrap; // +0x1DC8 debug trap switch
    u8 _pad_1DCC[0x1C]; // +0x1DCC
    u32 bUberBaalDead; // +0x1DE8 class 709 done
    u32 bUberDiabloDead; // +0x1DEC class 705 done
    u32 bUberMephistoDead; // +0x1DF0 class 704 done
};

/* ---- compiled .txt records (offsets from specs/data/fields.tsv, decimal there) ---- */

struct D2MonStatsTxt { // size 0x1A8, monstats.bin record; spec: specs/data/fields.tsv, specs/data/tables.tsv
    u16 wId; // +0x00 key(name16) monstats.Id
    u16 wBaseId; // +0x02 link16 monstats.Id
    u16 wNextInClass; // +0x04 link16 monstats.Id
    u16 wNameStr; // +0x06 strkey
    u16 wDescStr; // +0x08 strkey
    u8 _pad_0A[0x2]; // +0x0A
    u32 dwFlags0C; // +0x0C bits: isSpawn 0, isMelee 1, noRatio 2, opendoors 3, SetBoss 4, BossXfer 5, boss 6, primeevil 7, npc 8, interact 9, inTown 10, lUndead 11, hUndead 12, demon 13, flying 14, k
    u32 dwCode; // +0x10 code4
    u16 wMonSound; // +0x14 link16 monsounds.Id
    u16 wUMonSound; // +0x16 link16 monsounds.Id
    u16 wMonStatsEx; // +0x18 link16 monstats2.Id
    u16 wMonProp; // +0x1A link16 monprop.Id
    u16 wMonType; // +0x1C link16 montype.type
    u16 wAI; // +0x1E link16 monai.AI
    u16 wSpawn; // +0x20 link16 monstats.Id
    u8 nSpawnx; // +0x22 u8
    u8 nSpawny; // +0x23 u8
    u8 nSpawnmode; // +0x24 link8 monmode_lookup.code
    u8 _pad_25[0x1]; // +0x25
    u16 wMinion1; // +0x26 link16 monstats.Id
    u16 wMinion2; // +0x28 link16 monstats.Id
    u8 _pad_2A[0x2]; // +0x2A
    u8 nPartyMin; // +0x2C u8
    u8 nPartyMax; // +0x2D u8
    u8 nRarity; // +0x2E u8
    u8 nMinGrp; // +0x2F u8
    u8 nMaxGrp; // +0x30 u8
    u8 nSparsePopulate; // +0x31 u8
    u16 wVelocity; // +0x32 u16
    u16 wRun; // +0x34 u16
    u8 _pad_36[0x4]; // +0x36
    u16 wMissA1; // +0x3A link16 missiles.Missile
    u16 wMissA2; // +0x3C link16 missiles.Missile
    u16 wMissS1; // +0x3E link16 missiles.Missile
    u16 wMissS2; // +0x40 link16 missiles.Missile
    u16 wMissS3; // +0x42 link16 missiles.Missile
    u16 wMissS4; // +0x44 link16 missiles.Missile
    u16 wMissC; // +0x46 link16 missiles.Missile
    u16 wMissSQ; // +0x48 link16 missiles.Missile
    u8 _pad_4A[0x2]; // +0x4A
    u8 nAlign; // +0x4C u8
    u8 nTransLvl; // +0x4D u8
    u8 nThreat; // +0x4E u8
    u8 nAidel; // +0x4F u8
    u8 nAidelN; // +0x50 u8
    u8 nAidelH; // +0x51 u8
    u8 nAidist; // +0x52 u8
    u8 nAidistN; // +0x53 u8
    u8 nAidistH; // +0x54 u8
    u8 _pad_55[0x1]; // +0x55
    u16 wAip1; // +0x56 u16
    u16 wAip1N; // +0x58 u16
    u16 wAip1H; // +0x5A u16
    u16 wAip2; // +0x5C u16
    u16 wAip2N; // +0x5E u16
    u16 wAip2H; // +0x60 u16
    u16 wAip3; // +0x62 u16
    u16 wAip3N; // +0x64 u16
    u16 wAip3H; // +0x66 u16
    u16 wAip4; // +0x68 u16
    u16 wAip4N; // +0x6A u16
    u16 wAip4H; // +0x6C u16
    u16 wAip5; // +0x6E u16
    u16 wAip5N; // +0x70 u16
    u16 wAip5H; // +0x72 u16
    u16 wAip6; // +0x74 u16
    u16 wAip6N; // +0x76 u16
    u16 wAip6H; // +0x78 u16
    u16 wAip7; // +0x7A u16
    u16 wAip7N; // +0x7C u16
    u16 wAip7H; // +0x7E u16
    u16 wAip8; // +0x80 u16
    u16 wAip8N; // +0x82 u16
    u16 wAip8H; // +0x84 u16
    u16 wTreasureClass1; // +0x86 link16 @treasureclass
    u16 wTreasureClass2; // +0x88 link16 @treasureclass
    u16 wTreasureClass3; // +0x8A link16 @treasureclass
    u16 wTreasureClass4; // +0x8C link16 @treasureclass
    u16 wTreasureClass1N; // +0x8E link16 @treasureclass
    u16 wTreasureClass2N; // +0x90 link16 @treasureclass
    u16 wTreasureClass3N; // +0x92 link16 @treasureclass
    u16 wTreasureClass4N; // +0x94 link16 @treasureclass
    u16 wTreasureClass1H; // +0x96 link16 @treasureclass
    u16 wTreasureClass2H; // +0x98 link16 @treasureclass
    u16 wTreasureClass3H; // +0x9A link16 @treasureclass
    u16 wTreasureClass4H; // +0x9C link16 @treasureclass
    u8 nTCQuestId; // +0x9E u8
    u8 nTCQuestCP; // +0x9F u8
    u8 nDrain; // +0xA0 u8
    u8 nDrainN; // +0xA1 u8
    u8 nDrainH; // +0xA2 u8
    u8 nToBlock; // +0xA3 u8
    u8 nToBlockN; // +0xA4 u8
    u8 nToBlockH; // +0xA5 u8
    u8 nCrit; // +0xA6 u8
    u8 _pad_A7[0x1]; // +0xA7
    u16 wSkillDamage; // +0xA8 link16 skills.skill
    u16 wLevel; // +0xAA u16
    u16 wLevelN; // +0xAC u16
    u16 wLevelH; // +0xAE u16
    u16 wMinHP; // +0xB0 u16
    u16 wMinHPN; // +0xB2 u16
    u16 wMinHPH; // +0xB4 u16
    u16 wMaxHP; // +0xB6 u16
    u16 wMaxHPN; // +0xB8 u16
    u16 wMaxHPH; // +0xBA u16
    u16 wAC; // +0xBC u16
    u16 wACN; // +0xBE u16
    u16 wACH; // +0xC0 u16
    u16 wA1TH; // +0xC2 u16
    u16 wA1THN; // +0xC4 u16
    u16 wA1THH; // +0xC6 u16
    u16 wA2TH; // +0xC8 u16
    u16 wA2THN; // +0xCA u16
    u16 wA2THH; // +0xCC u16
    u16 wS1TH; // +0xCE u16
    u16 wS1THN; // +0xD0 u16
    u16 wS1THH; // +0xD2 u16
    u16 wExp; // +0xD4 u16
    u16 wExpN; // +0xD6 u16
    u16 wExpH; // +0xD8 u16
    u16 wA1MinD; // +0xDA u16
    u16 wA1MinDN; // +0xDC u16
    u16 wA1MinDH; // +0xDE u16
    u16 wA1MaxD; // +0xE0 u16
    u16 wA1MaxDN; // +0xE2 u16
    u16 wA1MaxDH; // +0xE4 u16
    u16 wA2MinD; // +0xE6 u16
    u16 wA2MinDN; // +0xE8 u16
    u16 wA2MinDH; // +0xEA u16
    u16 wA2MaxD; // +0xEC u16
    u16 wA2MaxDN; // +0xEE u16
    u16 wA2MaxDH; // +0xF0 u16
    u16 wS1MinD; // +0xF2 u16
    u16 wS1MinDN; // +0xF4 u16
    u16 wS1MinDH; // +0xF6 u16
    u16 wS1MaxD; // +0xF8 u16
    u16 wS1MaxDN; // +0xFA u16
    u16 wS1MaxDH; // +0xFC u16
    u8 nEl1Mode; // +0xFE link8 monmode_lookup.code
    u8 nEl2Mode; // +0xFF link8 monmode_lookup.code
    u8 nEl3Mode; // +0x100 link8 monmode_lookup.code
    u8 nEl1Type; // +0x101 link8 elemtypes.code
    u8 nEl2Type; // +0x102 link8 elemtypes.code
    u8 nEl3Type; // +0x103 link8 elemtypes.code
    u8 nEl1Pct; // +0x104 u8
    u8 nEl1PctN; // +0x105 u8
    u8 nEl1PctH; // +0x106 u8
    u8 nEl2Pct; // +0x107 u8
    u8 nEl2PctN; // +0x108 u8
    u8 nEl2PctH; // +0x109 u8
    u8 nEl3Pct; // +0x10A u8
    u8 nEl3PctN; // +0x10B u8
    u8 nEl3PctH; // +0x10C u8
    u8 _pad_10D[0x1]; // +0x10D
    u16 wEl1MinD; // +0x10E u16
    u16 wEl1MinDN; // +0x110 u16
    u16 wEl1MinDH; // +0x112 u16
    u16 wEl2MinD; // +0x114 u16
    u16 wEl2MinDN; // +0x116 u16
    u16 wEl2MinDH; // +0x118 u16
    u16 wEl3MinD; // +0x11A u16
    u16 wEl3MinDN; // +0x11C u16
    u16 wEl3MinDH; // +0x11E u16
    u16 wEl1MaxD; // +0x120 u16
    u16 wEl1MaxDN; // +0x122 u16
    u16 wEl1MaxDH; // +0x124 u16
    u16 wEl2MaxD; // +0x126 u16
    u16 wEl2MaxDN; // +0x128 u16
    u16 wEl2MaxDH; // +0x12A u16
    u16 wEl3MaxD; // +0x12C u16
    u16 wEl3MaxDN; // +0x12E u16
    u16 wEl3MaxDH; // +0x130 u16
    u16 wEl1Dur; // +0x132 u16
    u16 wEl1DurN; // +0x134 u16
    u16 wEl1DurH; // +0x136 u16
    u16 wEl2Dur; // +0x138 u16
    u16 wEl2DurN; // +0x13A u16
    u16 wEl2DurH; // +0x13C u16
    u16 wEl3Dur; // +0x13E u16
    u16 wEl3DurN; // +0x140 u16
    u16 wEl3DurH; // +0x142 u16
    u16 wResDm; // +0x144 u16
    u16 wResDmN; // +0x146 u16
    u16 wResDmH; // +0x148 u16
    u16 wResMa; // +0x14A u16
    u16 wResMaN; // +0x14C u16
    u16 wResMaH; // +0x14E u16
    u16 wResFi; // +0x150 u16
    u16 wResFiN; // +0x152 u16
    u16 wResFiH; // +0x154 u16
    u16 wResLi; // +0x156 u16
    u16 wResLiN; // +0x158 u16
    u16 wResLiH; // +0x15A u16
    u16 wResCo; // +0x15C u16
    u16 wResCoN; // +0x15E u16
    u16 wResCoH; // +0x160 u16
    u16 wResPo; // +0x162 u16
    u16 wResPoN; // +0x164 u16
    u16 wResPoH; // +0x166 u16
    u8 nColdEffect; // +0x168 u8
    u8 nColdEffectN; // +0x169 u8
    u8 nColdEffectH; // +0x16A u8
    u8 _pad_16B[0x1]; // +0x16B
    u32 dwSendSkills; // +0x16C u32
    u16 wSkill1; // +0x170 link16 skills.skill
    u16 wSkill2; // +0x172 link16 skills.skill
    u16 wSkill3; // +0x174 link16 skills.skill
    u16 wSkill4; // +0x176 link16 skills.skill
    u16 wSkill5; // +0x178 link16 skills.skill
    u16 wSkill6; // +0x17A link16 skills.skill
    u16 wSkill7; // +0x17C link16 skills.skill
    u16 wSkill8; // +0x17E link16 skills.skill
    u8 _pad_180[0x18]; // +0x180
    u8 nSk1lvl; // +0x198 u8
    u8 nSk2lvl; // +0x199 u8
    u8 nSk3lvl; // +0x19A u8
    u8 nSk4lvl; // +0x19B u8
    u8 nSk5lvl; // +0x19C u8
    u8 nSk6lvl; // +0x19D u8
    u8 nSk7lvl; // +0x19E u8
    u8 nSk8lvl; // +0x19F u8
    u32 dwDamageRegen; // +0x1A0 u32
    u8 nSplEndDeath; // +0x1A4 u8
    u8 nSplGetModeChart; // +0x1A5 u8
    u8 nSplEndGeneric; // +0x1A6 u8
    u8 nSplClientEnd; // +0x1A7 u8
};

struct D2MonStats2Txt { // size 0x134, monstats2.bin record; spec: specs/data/fields.tsv, specs/data/tables.tsv
    u16 wId; // +0x00 key(name16) monstats2.Id
    u8 _pad_02[0x2]; // +0x02
    u32 dwFlags04; // +0x04 bits: noGfxHitTest 0, noMap 1, noOvly 2, isSel 3, alSel 4, noSel 5, shiftSel 6, corpseSel 7, revive 8, isAtt 9, small 10, large 11, soft 12, critter 13, shadow 14, noUnique
    u8 nSizeX; // +0x08 u8
    u8 nSizeY; // +0x09 u8
    u8 nSpawnCol; // +0x0A u8
    u8 nHeight; // +0x0B u8
    u8 nOverlayHeight; // +0x0C u8
    u8 nPixHeight; // +0x0D u8
    u8 nMeleeRng; // +0x0E u8
    u8 _pad_0F[0x1]; // +0x0F
    u32 dwBaseW; // +0x10 code4
    u8 nHitClass; // +0x14 u8
    u8 _pad_15[0xD3]; // +0x15
    u32 dwFlagsE8; // +0xE8 bits: HD 0, TR 1, LG 2, RA 3, LA 4, RH 5, LH 6, SH 7, S1 8, S2 9, S3 10, S4 11, S5 12, S6 13, S7 14, S8 15
    u8 nTotalPieces; // +0xEC u8
    u8 _pad_ED[0x3]; // +0xED
    u32 dwFlagsF0; // +0xF0 bits: mDT 0, mNU 1, mWL 2, mGH 3, mA1 4, mA2 5, mBL 6, mSC 7, mS1 8, mS2 9, mS3 10, mS4 11, mDD 12, mKB 13, mSQ 14, mRN 15
    u8 nDDT; // +0xF4 u8
    u8 nDNU; // +0xF5 u8
    u8 nDWL; // +0xF6 u8
    u8 nDGH; // +0xF7 u8
    u8 nDA1; // +0xF8 u8
    u8 nDA2; // +0xF9 u8
    u8 nDBL; // +0xFA u8
    u8 nDSC; // +0xFB u8
    u8 nDS1; // +0xFC u8
    u8 nDS2; // +0xFD u8
    u8 nDS3; // +0xFE u8
    u8 nDS4; // +0xFF u8
    u8 nDDD; // +0x100 u8
    u8 nDKB; // +0x101 u8
    u8 nDSQ; // +0x102 u8
    u8 nDRN; // +0x103 u8
    u32 dwFlags104; // +0x104 bits: A1mv 4, A2mv 5, SCmv 7, S1mv 8, S2mv 9, S3mv 10, S4mv 11
    u8 nInfernoLen; // +0x108 u8
    u8 nInfernoAnim; // +0x109 u8
    u8 nInfernoRollback; // +0x10A u8
    u8 nResurrectMode; // +0x10B link8 monmode_lookup.code
    u16 wResurrectSkill; // +0x10C link16 skills.skill
    u16 wHtTop; // +0x10E u16
    u16 wHtLeft; // +0x110 u16
    u16 wHtWidth; // +0x112 u16
    u16 wHtHeight; // +0x114 u16
    u8 _pad_116[0x2]; // +0x116
    u32 dwAutomapCel; // +0x118 u32
    u8 nLocalBlood; // +0x11C u8
    u8 nBleed; // +0x11D u8
    u8 nLight; // +0x11E u8
    u8 nLightr; // +0x11F u8
    u8 nLightg; // +0x120 u8
    u8 nLightb; // +0x121 u8
    u8 nUtrans; // +0x122 u8
    u8 nUtransN; // +0x123 u8
    u8 nUtransH; // +0x124 u8
    u8 _pad_125[0x3]; // +0x125
    u32 dwHeart; // +0x128 code4
    u32 dwBodyPart; // +0x12C code4
    u8 nRestore; // +0x130 u8
    u8 _pad_131[0x3]; // +0x131
};

struct D2SkillsTxt { // size 0x23C, skills.bin record; spec: specs/data/fields.tsv, specs/data/tables.tsv
    u16 wSkill; // +0x00 key(name16) skills.skill
    u8 _pad_02[0x2]; // +0x02
    u32 dwFlags04; // +0x04 bits: decquant 0, lob 1, progressive 2, finishing 3, passive 4, aura 5, periodic 6, prgstack 7, InTown 8, Kick 9, InGame 10, repeat 11, stsuccessonly 12, stsounddelay 13, w
    u8 _pad_08[0x4]; // +0x08
    u8 nCharclass; // +0x0C link8 playerclass.code
    u8 _pad_0D[0x3]; // +0x0D
    u8 nAnim; // +0x10 link8 plrmode_lookup.code
    u8 nMonanim; // +0x11 link8 monmode_lookup.code
    u8 nSeqtrans; // +0x12 link8 plrmode_lookup.code
    u8 nSeqnum; // +0x13 u8
    u8 nRange; // +0x14 link8 @range
    u8 nSelectProc; // +0x15 u8
    u8 nSeqinput; // +0x16 u8
    u8 _pad_17[0x1]; // +0x17
    u16 wItypea1; // +0x18 link16 itemtypes.code
    u16 wItypea2; // +0x1A link16 itemtypes.code
    u16 wItypea3; // +0x1C link16 itemtypes.code
    u16 wItypeb1; // +0x1E link16 itemtypes.code
    u16 wItypeb2; // +0x20 link16 itemtypes.code
    u16 wItypeb3; // +0x22 link16 itemtypes.code
    u16 wEtypea1; // +0x24 link16 itemtypes.code
    u16 wEtypea2; // +0x26 link16 itemtypes.code
    u16 wEtypeb1; // +0x28 link16 itemtypes.code
    u16 wEtypeb2; // +0x2A link16 itemtypes.code
    u16 wSrvstfunc; // +0x2C u16
    u16 wSrvdofunc; // +0x2E u16
    u16 wSrvprgfunc1; // +0x30 u16
    u16 wSrvprgfunc2; // +0x32 u16
    u16 wSrvprgfunc3; // +0x34 u16
    u8 _pad_36[0x2]; // +0x36
    u32 dwPrgcalc1; // +0x38 cb calc(skillscode)
    u32 dwPrgcalc2; // +0x3C cb calc(skillscode)
    u32 dwPrgcalc3; // +0x40 cb calc(skillscode)
    u8 nPrgdam; // +0x44 u8
    u8 _pad_45[0x1]; // +0x45
    u16 wSrvmissile; // +0x46 link16 missiles.Missile
    u16 wSrvmissilea; // +0x48 link16 missiles.Missile
    u16 wSrvmissileb; // +0x4A link16 missiles.Missile
    u16 wSrvmissilec; // +0x4C link16 missiles.Missile
    u16 wSrvoverlay; // +0x4E link16 overlay.overlay
    u32 dwAurafilter; // +0x50 u32
    u16 wAurastat1; // +0x54 link16 itemstatcost.stat
    u16 wAurastat2; // +0x56 link16 itemstatcost.stat
    u16 wAurastat3; // +0x58 link16 itemstatcost.stat
    u16 wAurastat4; // +0x5A link16 itemstatcost.stat
    u16 wAurastat5; // +0x5C link16 itemstatcost.stat
    u16 wAurastat6; // +0x5E link16 itemstatcost.stat
    u32 dwAuralencalc; // +0x60 cb calc(skillscode)
    u32 dwAurarangecalc; // +0x64 cb calc(skillscode)
    u32 dwAurastatcalc1; // +0x68 cb calc(skillscode)
    u32 dwAurastatcalc2; // +0x6C cb calc(skillscode)
    u32 dwAurastatcalc3; // +0x70 cb calc(skillscode)
    u32 dwAurastatcalc4; // +0x74 cb calc(skillscode)
    u32 dwAurastatcalc5; // +0x78 cb calc(skillscode)
    u32 dwAurastatcalc6; // +0x7C cb calc(skillscode)
    u16 wAurastate; // +0x80 link16 states.state
    u16 wAuratargetstate; // +0x82 link16 states.state
    u16 wAuraevent1; // +0x84 link16 events.event
    u16 wAuraevent2; // +0x86 link16 events.event
    u16 wAuraevent3; // +0x88 link16 events.event
    u16 wAuraeventfunc1; // +0x8A u16
    u16 wAuraeventfunc2; // +0x8C u16
    u16 wAuraeventfunc3; // +0x8E u16
    u16 wAuratgtevent; // +0x90 link16 events.event
    u16 wAuratgteventfunc; // +0x92 u16
    u16 wPassivestate; // +0x94 link16 states.state
    u16 wPassiveitype; // +0x96 link16 itemtypes.code
    u16 wPassivestat1; // +0x98 link16 itemstatcost.stat
    u16 wPassivestat2; // +0x9A link16 itemstatcost.stat
    u16 wPassivestat3; // +0x9C link16 itemstatcost.stat
    u16 wPassivestat4; // +0x9E link16 itemstatcost.stat
    u16 wPassivestat5; // +0xA0 link16 itemstatcost.stat
    u8 _pad_A2[0x2]; // +0xA2
    u32 dwPassivecalc1; // +0xA4 cb calc(skillscode)
    u32 dwPassivecalc2; // +0xA8 cb calc(skillscode)
    u32 dwPassivecalc3; // +0xAC cb calc(skillscode)
    u32 dwPassivecalc4; // +0xB0 cb calc(skillscode)
    u32 dwPassivecalc5; // +0xB4 cb calc(skillscode)
    u16 wPassiveevent; // +0xB8 link16 events.event
    u16 wPassiveeventfunc; // +0xBA u16
    u16 wSummon; // +0xBC link16 monstats_lookup.Id
    u8 nPettype; // +0xBE link8 pettype.pet type
    u8 nSummode; // +0xBF link8 monmode_lookup.code
    u32 dwPetmax; // +0xC0 cb calc(skillscode)
    u16 wSumskill1; // +0xC4 link16 skills.skill
    u16 wSumskill2; // +0xC6 link16 skills.skill
    u16 wSumskill3; // +0xC8 link16 skills.skill
    u16 wSumskill4; // +0xCA link16 skills.skill
    u16 wSumskill5; // +0xCC link16 skills.skill
    u8 _pad_CE[0x2]; // +0xCE
    u32 dwSumsk1calc; // +0xD0 cb calc(skillscode)
    u32 dwSumsk2calc; // +0xD4 cb calc(skillscode)
    u32 dwSumsk3calc; // +0xD8 cb calc(skillscode)
    u32 dwSumsk4calc; // +0xDC cb calc(skillscode)
    u32 dwSumsk5calc; // +0xE0 cb calc(skillscode)
    u16 wSumumod; // +0xE4 u16
    u16 wSumoverlay; // +0xE6 link16 overlay.overlay
    u16 wCltmissile; // +0xE8 link16 missiles.Missile
    u16 wCltmissilea; // +0xEA link16 missiles.Missile
    u16 wCltmissileb; // +0xEC link16 missiles.Missile
    u16 wCltmissilec; // +0xEE link16 missiles.Missile
    u16 wCltmissiled; // +0xF0 link16 missiles.Missile
    u16 wCltstfunc; // +0xF2 u16
    u16 wCltdofunc; // +0xF4 u16
    u16 wCltprgfunc1; // +0xF6 u16
    u16 wCltprgfunc2; // +0xF8 u16
    u16 wCltprgfunc3; // +0xFA u16
    u16 wStsound; // +0xFC link16 sounds.Sound
    u16 wStsoundclass; // +0xFE link16 sounds.Sound
    u16 wDosound; // +0x100 link16 sounds.Sound
    u16 wDosounda; // +0x102 link16 sounds.Sound
    u16 wDosoundb; // +0x104 link16 sounds.Sound
    u16 wCastoverlay; // +0x106 link16 overlay.overlay
    u16 wTgtoverlay; // +0x108 link16 overlay.overlay
    u16 wTgtsound; // +0x10A link16 sounds.Sound
    u16 wPrgoverlay; // +0x10C link16 overlay.overlay
    u16 wPrgsound; // +0x10E link16 sounds.Sound
    u16 wCltoverlaya; // +0x110 link16 overlay.overlay
    u16 wCltoverlayb; // +0x112 link16 overlay.overlay
    u32 dwCltcalc1; // +0x114 cb calc(skillscode)
    u32 dwCltcalc2; // +0x118 cb calc(skillscode)
    u32 dwCltcalc3; // +0x11C cb calc(skillscode)
    u8 nItemTarget; // +0x120 u8
    u8 _pad_121[0x1]; // +0x121
    u16 wItemCastSound; // +0x122 link16 sounds.Sound
    u16 wItemCastOverlay; // +0x124 link16 overlay.overlay
    u8 _pad_126[0x2]; // +0x126
    u32 dwPerdelay; // +0x128 cb calc(skillscode)
    u16 wMaxlvl; // +0x12C u16
    u16 wResultFlags; // +0x12E u16
    u32 dwHitFlags; // +0x130 u32
    u32 dwHitClass; // +0x134 u32
    u32 dwCalc1; // +0x138 cb calc(skillscode)
    u32 dwCalc2; // +0x13C cb calc(skillscode)
    u32 dwCalc3; // +0x140 cb calc(skillscode)
    u32 dwCalc4; // +0x144 cb calc(skillscode)
    u32 dwParam1; // +0x148 u32
    u32 dwParam2; // +0x14C u32
    u32 dwParam3; // +0x150 u32
    u32 dwParam4; // +0x154 u32
    u32 dwParam5; // +0x158 u32
    u32 dwParam6; // +0x15C u32
    u32 dwParam7; // +0x160 u32
    u32 dwParam8; // +0x164 u32
    u8 nWeapsel; // +0x168 u8
    u8 _pad_169[0x1]; // +0x169
    u16 wItemEffect; // +0x16A u16
    u16 wItemCltEffect; // +0x16C u16
    u8 _pad_16E[0x2]; // +0x16E
    u32 dwSkpoints; // +0x170 cb calc(skillscode)
    u16 wReqlevel; // +0x174 u16
    u16 wReqstr; // +0x176 u16
    u16 wReqdex; // +0x178 u16
    u16 wReqint; // +0x17A u16
    u16 wReqvit; // +0x17C u16
    u16 wReqskill1; // +0x17E link16 skills.skill
    u16 wReqskill2; // +0x180 link16 skills.skill
    u16 wReqskill3; // +0x182 link16 skills.skill
    u16 wStartmana; // +0x184 u16
    u16 wMinmana; // +0x186 u16
    u16 wManashift; // +0x188 u16
    u16 wMana; // +0x18A u16
    u16 wLvlmana; // +0x18C u16
    u8 nAttackrank; // +0x18E u8
    u8 nLineofsight; // +0x18F u8
    u32 dwDelay; // +0x190 cb calc(skillscode)
    u16 wSkilldesc; // +0x194 link16 skilldesc_lookup.skilldesc
    u8 _pad_196[0x2]; // +0x196
    u32 dwToHit; // +0x198 u32
    u32 dwLevToHit; // +0x19C u32
    u32 dwToHitCalc; // +0x1A0 cb calc(skillscode)
    u8 nHitShift; // +0x1A4 u8
    u8 nSrcDam; // +0x1A5 u8
    u8 _pad_1A6[0x2]; // +0x1A6
    u32 dwMinDam; // +0x1A8 u32
    u32 dwMaxDam; // +0x1AC u32
    u32 dwMinLevDam1; // +0x1B0 u32
    u32 dwMinLevDam2; // +0x1B4 u32
    u32 dwMinLevDam3; // +0x1B8 u32
    u32 dwMinLevDam4; // +0x1BC u32
    u32 dwMinLevDam5; // +0x1C0 u32
    u32 dwMaxLevDam1; // +0x1C4 u32
    u32 dwMaxLevDam2; // +0x1C8 u32
    u32 dwMaxLevDam3; // +0x1CC u32
    u32 dwMaxLevDam4; // +0x1D0 u32
    u32 dwMaxLevDam5; // +0x1D4 u32
    u32 dwDmgSymPerCalc; // +0x1D8 cb calc(skillscode)
    u8 nEType; // +0x1DC link8 elemtypes.code
    u8 _pad_1DD[0x3]; // +0x1DD
    u32 dwEMin; // +0x1E0 u32
    u32 dwEMax; // +0x1E4 u32
    u32 dwEMinLev1; // +0x1E8 u32
    u32 dwEMinLev2; // +0x1EC u32
    u32 dwEMinLev3; // +0x1F0 u32
    u32 dwEMinLev4; // +0x1F4 u32
    u32 dwEMinLev5; // +0x1F8 u32
    u32 dwEMaxLev1; // +0x1FC u32
    u32 dwEMaxLev2; // +0x200 u32
    u32 dwEMaxLev3; // +0x204 u32
    u32 dwEMaxLev4; // +0x208 u32
    u32 dwEMaxLev5; // +0x20C u32
    u32 dwEDmgSymPerCalc; // +0x210 cb calc(skillscode)
    u32 dwELen; // +0x214 u32
    u32 dwELevLen1; // +0x218 u32
    u32 dwELevLen2; // +0x21C u32
    u32 dwELevLen3; // +0x220 u32
    u32 dwELenSymPerCalc; // +0x224 cb calc(skillscode)
    u8 nRestrict; // +0x228 u8
    u8 _pad_229[0x1]; // +0x229
    u16 wState1; // +0x22A link16 states.state
    u16 wState2; // +0x22C link16 states.state
    u16 wState3; // +0x22E link16 states.state
    u8 nAitype; // +0x230 u8
    u8 _pad_231[0x1]; // +0x231
    u16 wAibonus; // +0x232 u16
    u32 dwCostmult; // +0x234 u32
    u32 dwCostadd; // +0x238 u32
};

struct D2MissilesTxt { // size 0x1A4, missiles.bin record; spec: specs/data/fields.tsv, specs/missiles/missiles.md §R1
    u16 wMissile; // +0x00 key(name16) missiles.Missile
    u8 _pad_02[0x2]; // +0x02
    u32 dwFlags04; // +0x04 bits: LastCollide 0, Explosion 1, Pierce 2, CanSlow 3, CanDestroy 4, ClientSend 5, GetHit 6, SoftHit 7, ApplyMastery 8, ReturnFire 9, Town 10, SrcTown 11, NoMultiShot 12, N
    u16 wPCltDoFunc; // +0x08 u16
    u16 wPCltHitFunc; // +0x0A u16
    u16 wPSrvDoFunc; // +0x0C u16
    u16 wPSrvHitFunc; // +0x0E u16
    u16 wPSrvDmgFunc; // +0x10 u16
    u16 wTravelSound; // +0x12 link16 sounds.Sound
    u16 wHitSound; // +0x14 link16 sounds.Sound
    u16 wExplosionMissile; // +0x16 link16 missiles.Missile
    u16 wSubMissile1; // +0x18 link16 missiles.Missile
    u16 wSubMissile2; // +0x1A link16 missiles.Missile
    u16 wSubMissile3; // +0x1C link16 missiles.Missile
    u16 wCltSubMissile1; // +0x1E link16 missiles.Missile
    u16 wCltSubMissile2; // +0x20 link16 missiles.Missile
    u16 wCltSubMissile3; // +0x22 link16 missiles.Missile
    u16 wHitSubMissile1; // +0x24 link16 missiles.Missile
    u16 wHitSubMissile2; // +0x26 link16 missiles.Missile
    u16 wHitSubMissile3; // +0x28 link16 missiles.Missile
    u16 wHitSubMissile4; // +0x2A link16 missiles.Missile
    u16 wCltHitSubMissile1; // +0x2C link16 missiles.Missile
    u16 wCltHitSubMissile2; // +0x2E link16 missiles.Missile
    u16 wCltHitSubMissile3; // +0x30 link16 missiles.Missile
    u16 wCltHitSubMissile4; // +0x32 link16 missiles.Missile
    u16 wProgSound; // +0x34 link16 sounds.Sound
    u16 wProgOverlay; // +0x36 link16 overlay.overlay
    u32 dwParam1; // +0x38 u32
    u32 dwParam2; // +0x3C u32
    u32 dwParam3; // +0x40 u32
    u32 dwParam4; // +0x44 u32
    u32 dwParam5; // +0x48 u32
    u32 dwSHitPar1; // +0x4C u32
    u32 dwSHitPar2; // +0x50 u32
    u32 dwSHitPar3; // +0x54 u32
    u32 dwCltParam1; // +0x58 u32
    u32 dwCltParam2; // +0x5C u32
    u32 dwCltParam3; // +0x60 u32
    u32 dwCltParam4; // +0x64 u32
    u32 dwCltParam5; // +0x68 u32
    u32 dwCHitPar1; // +0x6C u32
    u32 dwCHitPar2; // +0x70 u32
    u32 dwCHitPar3; // +0x74 u32
    u32 dwDParam1; // +0x78 u32
    u32 dwDParam2; // +0x7C u32
    u32 dwSrvCalc1; // +0x80 cb calc(misscode)
    u32 dwCltCalc1; // +0x84 cb calc(misscode)
    u32 dwSHitCalc1; // +0x88 cb calc(misscode)
    u32 dwCHitCalc1; // +0x8C cb calc(misscode)
    u32 dwDmgCalc1; // +0x90 cb calc(misscode)
    u8 nHitClass; // +0x94 u8
    u8 _pad_95[0x1]; // +0x95
    u16 wRange; // +0x96 u16
    u16 wLevRange; // +0x98 u16
    u8 nVel; // +0x9A u8
    u8 nVelLev; // +0x9B u8
    u8 nMaxVel; // +0x9C u8
    u8 _pad_9D[0x1]; // +0x9D
    u16 wAccel; // +0x9E u16
    u16 wAnimrate; // +0xA0 u16
    u16 wXoffset; // +0xA2 u16
    u16 wYoffset; // +0xA4 u16
    u16 wZoffset; // +0xA6 u16
    u32 dwHitFlags; // +0xA8 u32
    u16 wResultFlags; // +0xAC u16
    u8 nKnockBack; // +0xAE u8
    u8 _pad_AF[0x1]; // +0xAF
    u32 dwMinDamage; // +0xB0 u32
    u32 dwMaxDamage; // +0xB4 u32
    u32 dwMinLevDam1; // +0xB8 u32
    u32 dwMinLevDam2; // +0xBC u32
    u32 dwMinLevDam3; // +0xC0 u32
    u32 dwMinLevDam4; // +0xC4 u32
    u32 dwMinLevDam5; // +0xC8 u32
    u32 dwMaxLevDam1; // +0xCC u32
    u32 dwMaxLevDam2; // +0xD0 u32
    u32 dwMaxLevDam3; // +0xD4 u32
    u32 dwMaxLevDam4; // +0xD8 u32
    u32 dwMaxLevDam5; // +0xDC u32
    u32 dwDmgSymPerCalc; // +0xE0 cb calc(misscode)
    u8 nEType; // +0xE4 link8 elemtypes.code
    u8 _pad_E5[0x3]; // +0xE5
    u32 dwEMin; // +0xE8 u32
    u32 dwEMax; // +0xEC u32
    u32 dwMinELev1; // +0xF0 u32
    u32 dwMinELev2; // +0xF4 u32
    u32 dwMinELev3; // +0xF8 u32
    u32 dwMinELev4; // +0xFC u32
    u32 dwMinELev5; // +0x100 u32
    u32 dwMaxELev1; // +0x104 u32
    u32 dwMaxELev2; // +0x108 u32
    u32 dwMaxELev3; // +0x10C u32
    u32 dwMaxELev4; // +0x110 u32
    u32 dwMaxELev5; // +0x114 u32
    u32 dwEDmgSymPerCalc; // +0x118 cb calc(misscode)
    u32 dwELen; // +0x11C u32
    u32 dwELevLen1; // +0x120 u32
    u32 dwELevLen2; // +0x124 u32
    u32 dwELevLen3; // +0x128 u32
    u8 nCltSrcTown; // +0x12C u8
    u8 nSrcDamage; // +0x12D u8
    u8 nSrcMissDmg; // +0x12E u8
    u8 nHoly; // +0x12F u8
    u8 nLight; // +0x130 u8
    u8 nFlicker; // +0x131 u8
    u8 nRed; // +0x132 u8
    u8 nGreen; // +0x133 u8
    u8 nBlue; // +0x134 u8
    u8 nInitSteps; // +0x135 u8
    u8 nActivate; // +0x136 u8
    u8 nLoopAnim; // +0x137 u8
    char szCelFile[64]; // +0x138 str
    u8 nAnimLen; // +0x178 u8
    u8 _pad_179[0x3]; // +0x179
    u32 dwRandStart; // +0x17C u32
    u8 nSubLoop; // +0x180 u8
    u8 nSubStart; // +0x181 u8
    u8 nSubStop; // +0x182 u8
    u8 nCollideType; // +0x183 u8
    u8 nCollision; // +0x184 u8
    u8 nClientCol; // +0x185 u8
    u8 nCollideKill; // +0x186 u8
    u8 nCollideFriend; // +0x187 u8
    u8 nNextHit; // +0x188 u8
    u8 nNextDelay; // +0x189 u8
    u8 nSize; // +0x18A u8
    u8 nToHit; // +0x18B u8
    u8 nAlwaysExplode; // +0x18C u8
    u8 nTrans; // +0x18D u8
    u8 nQty; // +0x18E u8
    u8 _pad_18F[0x1]; // +0x18F
    u32 dwSpecialSetup; // +0x190 u32
    u16 wSkill; // +0x194 link16 skills_lookup.skill
    u8 nHitShift; // +0x196 u8
    u8 _pad_197[0x5]; // +0x197
    u32 dwDamageRate; // +0x19C u32
    u8 nNumDirections; // +0x1A0 u8
    u8 nAnimSpeed; // +0x1A1 u8
    u8 nLocalBlood; // +0x1A2 u8
    u8 _pad_1A3[0x1]; // +0x1A3
};

struct D2ItemsTxt { // size 0x1A8, weapons/armor/misc.bin record (one items array); spec: specs/data/fields.tsv, specs/data/tables.tsv
    char szFlippyfile[31]; // +0x00 str
    u8 _pad_1F[0x1]; // +0x1F
    char szInvfile[31]; // +0x20 str
    u8 _pad_3F[0x1]; // +0x3F
    char szUniqueinvfile[31]; // +0x40 str
    u8 _pad_5F[0x1]; // +0x5F
    char szSetinvfile[31]; // +0x60 str
    u8 _pad_7F[0x1]; // +0x7F
    u32 dwCode; // +0x80 key(code4) items.code
    u32 dwNormcode; // +0x84 code4
    u32 dwUbercode; // +0x88 code4
    u32 dwUltracode; // +0x8C code4
    u32 dwAlternategfx; // +0x90 code4
    u32 dwPSpell; // +0x94 u32
    u16 wState; // +0x98 link16 states.state
    u16 wCstate1; // +0x9A link16 states.state
    u16 wCstate2; // +0x9C link16 states.state
    u16 wStat1; // +0x9E link16 itemstatcost.stat
    u16 wStat2; // +0xA0 link16 itemstatcost.stat
    u16 wStat3; // +0xA2 link16 itemstatcost.stat
    u32 dwCalc1; // +0xA4 cb calc(itemscode)
    u32 dwCalc2; // +0xA8 cb calc(itemscode)
    u32 dwCalc3; // +0xAC cb calc(itemscode)
    u32 dwLen; // +0xB0 cb calc(itemscode)
    u8 nSpelldesc; // +0xB4 u8
    u8 _pad_B5[0x1]; // +0xB5
    u16 wSpelldescstr; // +0xB6 strkey
    u32 dwSpelldesccalc; // +0xB8 cb calc(itemscode)
    u32 dwBetterGem; // +0xBC code4
    u32 dwWclass; // +0xC0 code4
    u32 dw2handedwclass; // +0xC4 code4
    u32 dwTMogType; // +0xC8 code4
    u32 dwMinac; // +0xCC u32
    u32 dwMaxac; // +0xD0 u32
    u32 dwGamblecost; // +0xD4 u32
    u32 dwSpeed; // +0xD8 u32
    u32 dwBitfield1; // +0xDC u32
    u32 dwCost; // +0xE0 u32
    u32 dwMinstack; // +0xE4 u32
    u32 dwMaxstack; // +0xE8 u32
    u32 dwSpawnstack; // +0xEC u32
    u32 dwGemoffset; // +0xF0 u32
    u16 wNamestr; // +0xF4 strkey
    u16 wVersion; // +0xF6 u16
    u16 wAutoprefix; // +0xF8 u16
    u16 wMissiletype; // +0xFA u16
    u8 nRarity; // +0xFC u8
    u8 nLevel; // +0xFD u8
    u8 nMindam; // +0xFE u8
    u8 nMaxdam; // +0xFF u8
    u8 nMinmisdam; // +0x100 u8
    u8 nMaxmisdam; // +0x101 u8
    u8 n2handmindam; // +0x102 u8
    u8 n2handmaxdam; // +0x103 u8
    u8 nRangeadder; // +0x104 u8
    u8 _pad_105[0x1]; // +0x105
    u16 wStrbonus; // +0x106 u16
    u16 wDexbonus; // +0x108 u16
    u16 wReqstr; // +0x10A u16
    u16 wReqdex; // +0x10C u16
    u8 nAbsorbs; // +0x10E u8
    u8 nInvwidth; // +0x10F u8
    u8 nInvheight; // +0x110 u8
    u8 nBlock; // +0x111 u8
    u8 nDurability; // +0x112 u8
    u8 nNodurability; // +0x113 u8
    u8 nMissile; // +0x114 u8
    u8 nComponent; // +0x115 u8
    u8 nRArm; // +0x116 u8
    u8 nLArm; // +0x117 u8
    u8 nTorso; // +0x118 u8
    u8 nLegs; // +0x119 u8
    u8 nRspad; // +0x11A u8
    u8 nLspad; // +0x11B u8
    u8 n2handed; // +0x11C u8
    u8 nUseable; // +0x11D u8
    u16 wType; // +0x11E link16 itemtypes.code
    u16 wType2; // +0x120 link16 itemtypes.code
    u8 nSubtype; // +0x122 u8
    u8 _pad_123[0x1]; // +0x123
    u16 wDropsound; // +0x124 link16 sounds.Sound
    u16 wUsesound; // +0x126 link16 sounds.Sound
    u8 nDropsfxframe; // +0x128 u8
    u8 nUnique; // +0x129 u8
    u8 nQuest; // +0x12A u8
    u8 nQuestdiffcheck; // +0x12B u8
    u8 nTransparent; // +0x12C u8
    u8 nTranstbl; // +0x12D u8
    u8 _pad_12E[0x1]; // +0x12E
    u8 nLightradius; // +0x12F u8
    u8 nBelt; // +0x130 u8
    u8 nAutobelt; // +0x131 u8
    u8 nStackable; // +0x132 u8
    u8 nSpawnable; // +0x133 u8
    u8 nSpellicon; // +0x134 u8
    u8 nDurwarning; // +0x135 u8
    u8 nQntwarning; // +0x136 u8
    u8 nHasinv; // +0x137 u8
    u8 nGemsockets; // +0x138 u8
    u8 nTransmogrify; // +0x139 u8
    u8 nTMogMin; // +0x13A u8
    u8 nTMogMax; // +0x13B u8
    u8 nHitclass; // +0x13C link8 hitclass.code
    u8 n1or2handed; // +0x13D u8
    u8 nGemapplytype; // +0x13E u8
    u8 nLevelreq; // +0x13F u8
    u8 nMagiclvl; // +0x140 u8
    u8 nTransform; // +0x141 u8
    u8 nInvTrans; // +0x142 u8
    u8 nCompactsave; // +0x143 u8
    u8 nSkipName; // +0x144 u8
    u8 nNameable; // +0x145 u8
    u8 nAkaraMin; // +0x146 u8
    u8 nGheedMin; // +0x147 u8
    u8 nCharsiMin; // +0x148 u8
    u8 nFaraMin; // +0x149 u8
    u8 nLysanderMin; // +0x14A u8
    u8 nDrognanMin; // +0x14B u8
    u8 nHraltiMin; // +0x14C u8
    u8 nAlkorMin; // +0x14D u8
    u8 nOrmusMin; // +0x14E u8
    u8 nElzixMin; // +0x14F u8
    u8 nAshearaMin; // +0x150 u8
    u8 nCainMin; // +0x151 u8
    u8 nHalbuMin; // +0x152 u8
    u8 nJamellaMin; // +0x153 u8
    u8 nMalahMin; // +0x154 u8
    u8 nLarzukMin; // +0x155 u8
    u8 nDrehyaMin; // +0x156 u8
    u8 nAkaraMax; // +0x157 u8
    u8 nGheedMax; // +0x158 u8
    u8 nCharsiMax; // +0x159 u8
    u8 nFaraMax; // +0x15A u8
    u8 nLysanderMax; // +0x15B u8
    u8 nDrognanMax; // +0x15C u8
    u8 nHraltiMax; // +0x15D u8
    u8 nAlkorMax; // +0x15E u8
    u8 nOrmusMax; // +0x15F u8
    u8 nElzixMax; // +0x160 u8
    u8 nAshearaMax; // +0x161 u8
    u8 nCainMax; // +0x162 u8
    u8 nHalbuMax; // +0x163 u8
    u8 nJamellaMax; // +0x164 u8
    u8 nMalahMax; // +0x165 u8
    u8 nLarzukMax; // +0x166 u8
    u8 nDrehyaMax; // +0x167 u8
    u8 nAkaraMagicMin; // +0x168 u8
    u8 nGheedMagicMin; // +0x169 u8
    u8 nCharsiMagicMin; // +0x16A u8
    u8 nFaraMagicMin; // +0x16B u8
    u8 nLysanderMagicMin; // +0x16C u8
    u8 nDrognanMagicMin; // +0x16D u8
    u8 nHraltiMagicMin; // +0x16E u8
    u8 nAlkorMagicMin; // +0x16F u8
    u8 nOrmusMagicMin; // +0x170 u8
    u8 nElzixMagicMin; // +0x171 u8
    u8 nAshearaMagicMin; // +0x172 u8
    u8 nCainMagicMin; // +0x173 u8
    u8 nHalbuMagicMin; // +0x174 u8
    u8 nJamellaMagicMin; // +0x175 u8
    u8 nMalahMagicMin; // +0x176 u8
    u8 nLarzukMagicMin; // +0x177 u8
    u8 nDrehyaMagicMin; // +0x178 u8
    u8 nAkaraMagicMax; // +0x179 u8
    u8 nGheedMagicMax; // +0x17A u8
    u8 nCharsiMagicMax; // +0x17B u8
    u8 nFaraMagicMax; // +0x17C u8
    u8 nLysanderMagicMax; // +0x17D u8
    u8 nDrognanMagicMax; // +0x17E u8
    u8 nHraltiMagicMax; // +0x17F u8
    u8 nAlkorMagicMax; // +0x180 u8
    u8 nOrmusMagicMax; // +0x181 u8
    u8 nElzixMagicMax; // +0x182 u8
    u8 nAshearaMagicMax; // +0x183 u8
    u8 nCainMagicMax; // +0x184 u8
    u8 nHalbuMagicMax; // +0x185 u8
    u8 nJamellaMagicMax; // +0x186 u8
    u8 nMalahMagicMax; // +0x187 u8
    u8 nLarzukMagicMax; // +0x188 u8
    u8 nDrehyaMagicMax; // +0x189 u8
    u8 nAkaraMagicLvl; // +0x18A u8
    u8 nGheedMagicLvl; // +0x18B u8
    u8 nCharsiMagicLvl; // +0x18C u8
    u8 nFaraMagicLvl; // +0x18D u8
    u8 nLysanderMagicLvl; // +0x18E u8
    u8 nDrognanMagicLvl; // +0x18F u8
    u8 nHraltiMagicLvl; // +0x190 u8
    u8 nAlkorMagicLvl; // +0x191 u8
    u8 nOrmusMagicLvl; // +0x192 u8
    u8 nElzixMagicLvl; // +0x193 u8
    u8 nAshearaMagicLvl; // +0x194 u8
    u8 nCainMagicLvl; // +0x195 u8
    u8 nHalbuMagicLvl; // +0x196 u8
    u8 nJamellaMagicLvl; // +0x197 u8
    u8 nMalahMagicLvl; // +0x198 u8
    u8 nLarzukMagicLvl; // +0x199 u8
    u8 nDrehyaMagicLvl; // +0x19A u8
    u8 _pad_19B[0x1]; // +0x19B
    u32 dwNightmareUpgrade; // +0x19C code4
    u32 dwHellUpgrade; // +0x1A0 code4
    u8 nPermStoreItem; // +0x1A4 u8
    u8 nMultibuy; // +0x1A5 u8
    u8 _pad_1A6[0x2]; // +0x1A6
};

