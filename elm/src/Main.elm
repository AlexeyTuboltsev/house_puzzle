port module Main exposing (main)

import Browser
import Browser.Dom
import Dict exposing (Dict)
import File exposing (File)
import File.Select as Select
import Html exposing (..)
import Html.Attributes exposing (..)
import Html.Events exposing (..)
import Http
import Json.Decode as D
import Json.Encode as E
import Svg
import Svg.Attributes as SA
import Task



-- ── Types ──────────────────────────────────────────────────────────────────


type alias Point =
    ( Float, Float )


type alias Brick =
    { id : Int
    , x : Float
    , y : Float
    , width : Float
    , height : Float
    , brickType : String
    , neighbors : List Int
    , polygon : List Point
    }


type alias BrickRef =
    { id : Int
    , x : Float
    , y : Float
    , width : Float
    , height : Float
    }


type alias Piece =
    { id : Int
    , x : Float
    , y : Float
    , width : Float
    , height : Float
    , brickIds : List Int
    , bricks : List BrickRef
    , polygon : List Point
    }


type alias Canvas =
    { width : Float
    , height : Float
    }


type alias LoadResponse =
    { canvas : Canvas
    , bricks : List Brick
    , hasComposite : Bool
    , hasBase : Bool
    }


type alias MergeResponse =
    { pieces : List Piece
    }


type alias Wave =
    { id : Int
    , name : String
    , visible : Bool
    , locked : Bool
    , pieceIds : List Int
    }


type AppMode
    = ModeInit
    | ModeTif
    | ModePieces
    | ModeBlueprint
    | ModeWaves
    | ModeExport



-- ── Model ───────────────────────────────────────────────────────────────────


type LoadState
    = Idle
    | Uploading
    | Loading
    | Loaded LoadResponse
    | LoadError String


type GenerateState
    = NotGenerated
    | Compositing
    | Generated


type alias Model =
    { selectedFileName : String
    , loadState : LoadState
    , targetCount : Int
    , minBorder : Int
    , seed : Int
    , generateState : GenerateState
    , pieces : List Piece
    , pieceImages : Dict Int String
    , bricksById : Dict Int Brick
    , appMode : AppMode
    , showOutlines : Bool
    , showGrid : Bool
    , waves : List Wave
    , nextWaveId : Int
    , hoveredPieceId : Maybe Int
    , selectedPieceId : Maybe Int
    , selectedWaveId : Maybe Int
    , editMode : Bool
    , editBrickIds : List Int
    , editOriginalBrickIds : List Int
    , recomputing : Bool
    , exporting : Bool
    , draggingPieceId : Maybe Int
    , dragOverWaveId : Maybe (Maybe Int)
    , dragInsertBeforeId : Maybe Int
    , svgScale : Float
    }


init : () -> ( Model, Cmd Msg )
init _ =
    ( { selectedFileName = ""
      , loadState = Idle
      , targetCount = 60
      , minBorder = 5
      , seed = 42
      , generateState = NotGenerated
      , pieces = []
      , pieceImages = Dict.empty
      , bricksById = Dict.empty
      , appMode = ModeInit
      , showOutlines = True
      , showGrid = False
      , waves = []
      , nextWaveId = 1
      , hoveredPieceId = Nothing
      , selectedPieceId = Nothing
      , selectedWaveId = Nothing
      , editMode = False
      , editBrickIds = []
      , editOriginalBrickIds = []
      , recomputing = False
      , exporting = False
      , draggingPieceId = Nothing
      , dragOverWaveId = Nothing
      , dragInsertBeforeId = Nothing
      , svgScale = 1.0
      }
    , Cmd.none
    )




-- ── Msg ─────────────────────────────────────────────────────────────────────


type Msg
    = PickFile
    | FileSelected File
    | GotUploadResponse (Result Http.Error String)
    | GotLoadResponse (Result Http.Error LoadResponse)
    | SetTargetCount String
    | SetMinBorder String
    | SetSeed String
    | RequestGenerate
    | GotMergeResponse (Result Http.Error MergeResponse)
    | GotPieceImages E.Value
    | GotViewport Browser.Dom.Viewport
    | SetAppMode AppMode
    | ToggleOutlines Bool
    | ToggleGrid Bool
    | AddWave
    | ToggleWaveVisibility Int
    | SetHoveredPiece (Maybe Int)
    | SelectPiece Int
    | SelectAndEdit Int
    | SelectWave (Maybe Int)
    | AssignPieceToWave Int
    | RemovePieceFromWave Int Int
    | MoveWave Int Int
    | RemoveWave Int
    | StartEdit
    | ToggleBrickInEdit Int
    | SaveEdit
    | CancelEdit
    | GotPiecePolygons (Result Http.Error (List ( Int, List Point )))
    | RequestExport
    | ExportDone
    | LogBrickClick Int
    | DragPieceStart Int
    | DragPieceEnd
    | DragEnterWave (Maybe Int)
    | DragEnterPiece Int
    | DropOnWave (Maybe Int)
    | ToggleWaveLock Int
    | NoOp



-- ── Ports ───────────────────────────────────────────────────────────────────


port compositePieces : E.Value -> Cmd msg


port gotPieceImages : (E.Value -> msg) -> Sub msg


port exportZip : E.Value -> Cmd msg


port gotExportDone : (Bool -> msg) -> Sub msg


port logBrick : E.Value -> Cmd msg



-- ── Update ──────────────────────────────────────────────────────────────────


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        PickFile ->
            ( model, Select.file [ ".tif", "image/tiff" ] FileSelected )

        FileSelected file ->
            ( { model
                | selectedFileName = File.name file
                , loadState = Uploading
                , generateState = NotGenerated
                , pieces = []
                , pieceImages = Dict.empty
                , waves = []
                , nextWaveId = 1
                , selectedPieceId = Nothing
                , selectedWaveId = Nothing
                , editMode = False
                , editBrickIds = []
                , editOriginalBrickIds = []
                , recomputing = False
                , appMode = ModeInit
              }
            , uploadTif file
            )

        GotUploadResponse (Ok path) ->
            ( { model | loadState = Loading }, loadTif path )

        GotUploadResponse (Err _) ->
            ( { model | loadState = LoadError "Upload failed" }, Cmd.none )

        GotLoadResponse (Ok response) ->
            ( { model
                | loadState = Loaded response
                , bricksById =
                    response.bricks
                        |> List.map (\b -> ( b.id, b ))
                        |> Dict.fromList
                , appMode = ModeTif
              }
            , Cmd.none
            )

        GotLoadResponse (Err err) ->
            ( { model | loadState = LoadError (httpErrorToString err) }, Cmd.none )

        SetTargetCount s ->
            case String.toInt s of
                Just n ->
                    ( { model | targetCount = Basics.max 1 n }, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )

        SetMinBorder s ->
            case String.toInt s of
                Just n ->
                    ( { model | minBorder = Basics.max 0 n }, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )

        SetSeed s ->
            case String.toInt s of
                Just n ->
                    ( { model | seed = Basics.max 0 n }, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )

        RequestGenerate ->
            case model.loadState of
                Loaded _ ->
                    ( { model
                        | generateState = Compositing
                        , pieces = []
                        , pieceImages = Dict.empty
                        , waves = []
                        , nextWaveId = 1
                        , selectedPieceId = Nothing
                        , selectedWaveId = Nothing
                        , editMode = False
                        , editBrickIds = []
                        , editOriginalBrickIds = []
                        , recomputing = False
                      }
                    , mergeBricks model.targetCount model.minBorder model.seed
                    )

                _ ->
                    ( model, Cmd.none )

        GotMergeResponse (Ok response) ->
            ( { model | pieces = response.pieces }
            , compositePieces (encodePieceList response.pieces)
            )

        GotMergeResponse (Err _) ->
            ( { model | generateState = NotGenerated }, Cmd.none )

        GotPieceImages val ->
            case D.decodeValue decodePieceImages val of
                Ok images ->
                    ( { model
                        | pieceImages = Dict.union (Dict.fromList images) model.pieceImages
                        , generateState = Generated
                        , appMode = ModePieces
                      }
                    , Task.perform GotViewport Browser.Dom.getViewport
                    )

                Err _ ->
                    ( { model | generateState = NotGenerated }, Cmd.none )

        SetAppMode mode ->
            let
                baseModel =
                    { model | appMode = mode }
            in
            if mode == ModeWaves then
                case model.waves of
                    [] ->
                        let
                            newWave =
                                { id = model.nextWaveId
                                , name = "Wave " ++ String.fromInt model.nextWaveId
                                , visible = True
                                , locked = False
                                , pieceIds = []
                                }
                        in
                        ( { baseModel | waves = [ newWave ], nextWaveId = model.nextWaveId + 1, selectedWaveId = Just newWave.id }, Cmd.none )

                    first :: _ ->
                        ( { baseModel | selectedWaveId = if baseModel.selectedWaveId == Nothing then Just first.id else baseModel.selectedWaveId }
                        , Task.perform GotViewport Browser.Dom.getViewport
                        )

            else
                ( baseModel, Cmd.none )

        ToggleOutlines checked ->
            ( { model | showOutlines = checked }, Cmd.none )

        ToggleGrid checked ->
            ( { model | showGrid = checked }, Cmd.none )

        AddWave ->
            let
                newWave =
                    { id = model.nextWaveId
                    , name = "Wave " ++ String.fromInt model.nextWaveId
                    , visible = True
                    , locked = False
                    , pieceIds = []
                    }
            in
            ( { model
                | waves = model.waves ++ [ newWave ]
                , nextWaveId = model.nextWaveId + 1
                , selectedWaveId = Just newWave.id
              }
            , Cmd.none
            )

        ToggleWaveVisibility waveId ->
            ( { model
                | waves =
                    List.map
                        (\w ->
                            if w.id == waveId then
                                { w | visible = not w.visible }

                            else
                                w
                        )
                        model.waves
              }
            , Cmd.none
            )

        SetHoveredPiece mid ->
            ( { model | hoveredPieceId = mid }, Cmd.none )

        SelectPiece pid ->
            ( { model
                | selectedPieceId =
                    if model.selectedPieceId == Just pid then
                        Nothing

                    else
                        Just pid
              }
            , Cmd.none
            )

        SelectAndEdit pid ->
            case List.filter (\p -> p.id == pid) model.pieces |> List.head of
                Nothing ->
                    ( model, Cmd.none )

                Just piece ->
                    ( { model
                        | selectedPieceId = Just pid
                        , editMode = True
                        , editBrickIds = piece.brickIds
                        , editOriginalBrickIds = piece.brickIds
                      }
                    , Cmd.none
                    )

        SelectWave mwid ->
            ( { model | selectedWaveId = mwid }, Cmd.none )

        AssignPieceToWave pid ->
            case model.selectedWaveId of
                Nothing ->
                    ( model, Cmd.none )

                Just wid ->
                    let
                        targetWave =
                            model.waves |> List.filter (\w -> w.id == wid) |> List.head

                        targetLocked =
                            targetWave |> Maybe.map .locked |> Maybe.withDefault False

                        alreadyIn =
                            targetWave |> Maybe.map (\w -> List.member pid w.pieceIds) |> Maybe.withDefault False

                        sourceLocked =
                            model.waves |> List.any (\w -> w.locked && List.member pid w.pieceIds)

                        updatedWaves =
                            if targetLocked || (not alreadyIn && sourceLocked) then
                                model.waves

                            else
                                List.map
                                    (\w ->
                                        if w.id == wid then
                                            if alreadyIn then
                                                { w | pieceIds = List.filter (\p -> p /= pid) w.pieceIds }

                                            else
                                                { w | pieceIds = w.pieceIds ++ [ pid ] }

                                        else if not alreadyIn then
                                            -- adding to wid: remove from all other waves
                                            { w | pieceIds = List.filter (\p -> p /= pid) w.pieceIds }

                                        else
                                            w
                                    )
                                    model.waves
                    in
                    ( { model | waves = updatedWaves }, Cmd.none )

        RemovePieceFromWave wid pid ->
            let
                waveLocked =
                    model.waves |> List.any (\w -> w.id == wid && w.locked)
            in
            if waveLocked then
                ( model, Cmd.none )

            else
            ( { model
                | waves =
                    List.map
                        (\w ->
                            if w.id == wid then
                                { w | pieceIds = List.filter (\p -> p /= pid) w.pieceIds }

                            else
                                w
                        )
                        model.waves
              }
            , Cmd.none
            )

        MoveWave wid dir ->
            let
                indexed =
                    List.indexedMap Tuple.pair model.waves

                maybeIdx =
                    indexed
                        |> List.filter (\( _, w ) -> w.id == wid)
                        |> List.head
                        |> Maybe.map Tuple.first

                swapped =
                    case maybeIdx of
                        Nothing ->
                            model.waves

                        Just i ->
                            let
                                j =
                                    i + dir

                                n =
                                    List.length model.waves
                            in
                            if j < 0 || j >= n then
                                model.waves

                            else
                                List.indexedMap
                                    (\k w ->
                                        if k == i then
                                            Maybe.withDefault w (List.head (List.drop j model.waves))

                                        else if k == j then
                                            Maybe.withDefault w (List.head (List.drop i model.waves))

                                        else
                                            w
                                    )
                                    model.waves

                renumbered =
                    List.indexedMap (\i w -> { w | name = "Wave " ++ String.fromInt (i + 1) }) swapped
            in
            ( { model | waves = renumbered }, Cmd.none )

        RemoveWave wid ->
            let
                filtered =
                    List.filter (\w -> w.id /= wid) model.waves

                renumbered =
                    List.indexedMap (\i w -> { w | name = "Wave " ++ String.fromInt (i + 1) }) filtered

                newSelectedWaveId =
                    if model.selectedWaveId == Just wid then
                        Nothing

                    else
                        model.selectedWaveId
            in
            ( { model | waves = renumbered, selectedWaveId = newSelectedWaveId }, Cmd.none )

        StartEdit ->
            case model.selectedPieceId of
                Nothing ->
                    ( model, Cmd.none )

                Just pid ->
                    case List.filter (\p -> p.id == pid) model.pieces |> List.head of
                        Nothing ->
                            ( model, Cmd.none )

                        Just piece ->
                            ( { model
                                | editMode = True
                                , editBrickIds = piece.brickIds
                                , editOriginalBrickIds = piece.brickIds
                              }
                            , Cmd.none
                            )

        ToggleBrickInEdit bid ->
            let
                newList =
                    if List.member bid model.editBrickIds then
                        if List.length model.editBrickIds <= 1 then
                            model.editBrickIds

                        else
                            List.filter (\b -> b /= bid) model.editBrickIds

                    else
                        model.editBrickIds ++ [ bid ]
            in
            ( { model | editBrickIds = newList }, Cmd.none )

        SaveEdit ->
            case model.selectedPieceId of
                Nothing ->
                    ( { model | editMode = False, editBrickIds = [], editOriginalBrickIds = [] }
                    , Cmd.none
                    )

                Just editedPieceId ->
                    let
                        newBrickIds =
                            model.editBrickIds

                        removedBrickIds =
                            model.pieces
                                |> List.filter (\p -> p.id == editedPieceId)
                                |> List.head
                                |> Maybe.map (\p -> List.filter (\bid -> not (List.member bid newBrickIds)) p.brickIds)
                                |> Maybe.withDefault []

                        -- Update edited piece; strip stolen bricks from all others
                        updatedExisting =
                            List.map
                                (\p ->
                                    if p.id == editedPieceId then
                                        { p | brickIds = newBrickIds }

                                    else
                                        { p | brickIds = List.filter (\bid -> not (List.member bid newBrickIds)) p.brickIds }
                                )
                                model.pieces

                        -- New single-brick pieces for bricks removed from the edited piece
                        maxId =
                            List.foldl Basics.max 0 (List.map .id model.pieces)

                        newSinglePieces =
                            List.indexedMap
                                (\i bid ->
                                    case Dict.get bid model.bricksById of
                                        Just brick ->
                                            { id = maxId + i + 1
                                            , x = brick.x
                                            , y = brick.y
                                            , width = brick.width
                                            , height = brick.height
                                            , brickIds = [ bid ]
                                            , bricks = [ BrickRef bid brick.x brick.y brick.width brick.height ]
                                            , polygon = []
                                            }

                                        Nothing ->
                                            { id = maxId + i + 1
                                            , x = 0
                                            , y = 0
                                            , width = 0
                                            , height = 0
                                            , brickIds = [ bid ]
                                            , bricks = []
                                            , polygon = []
                                            }
                                )
                                removedBrickIds

                        -- Combine, filter empty, recalculate bboxes (keep original IDs)
                        allPieces =
                            (updatedExisting ++ newSinglePieces)
                                |> List.filter (\p -> not (List.isEmpty p.brickIds))
                                |> List.map (recalcPieceBbox model.bricksById)

                        -- Prune stale wave piece references
                        validIds =
                            List.map .id allPieces

                        updatedWaves =
                            List.map
                                (\w -> { w | pieceIds = List.filter (\pid -> List.member pid validIds) w.pieceIds })
                                model.waves
                    in
                    ( { model
                        | pieces = allPieces
                        , waves = updatedWaves
                        , editMode = False
                        , editBrickIds = []
                        , editOriginalBrickIds = []
                        , generateState = Generated
                        , recomputing = True
                        , selectedPieceId = Just editedPieceId
                      }
                    , Cmd.batch
                        [ compositePieces (encodePieceList allPieces)
                        , recomputePiecePolygons allPieces
                        ]
                    )

        CancelEdit ->
            ( { model
                | editMode = False
                , editBrickIds = []
                , editOriginalBrickIds = []
              }
            , Cmd.none
            )

        GotPiecePolygons (Ok pairs) ->
            let
                polyDict =
                    Dict.fromList pairs

                updatedPieces =
                    List.map
                        (\p ->
                            case Dict.get p.id polyDict of
                                Just poly ->
                                    { p | polygon = poly }

                                Nothing ->
                                    p
                        )
                        model.pieces
            in
            ( { model | pieces = updatedPieces, recomputing = False }, Cmd.none )

        GotPiecePolygons (Err _) ->
            ( { model | recomputing = False }, Cmd.none )

        RequestExport ->
            let
                wavesJson =
                    E.list
                        (\( idx, wv ) ->
                            E.object
                                [ ( "wave", E.int (idx + 1) )
                                , ( "pieceIds", E.list E.int wv.pieceIds )
                                ]
                        )
                        (List.indexedMap Tuple.pair model.waves)

                outlinesJson =
                    E.list
                        (\piece ->
                            E.object
                                [ ( "points"
                                  , E.list
                                        (\( x, y ) ->
                                            E.list E.float [ x, y ]
                                        )
                                        piece.polygon
                                  )
                                ]
                        )
                        model.pieces

                payload =
                    E.object
                        [ ( "waves", wavesJson )
                        , ( "outlines", outlinesJson )
                        , ( "placement"
                          , E.object
                                [ ( "location", E.string "Rome" )
                                , ( "position", E.int 0 )
                                , ( "houseName", E.string "NewHouse" )
                                , ( "spacing", E.float 12.0 )
                                ]
                          )
                        ]
            in
            ( { model | exporting = True }, exportZip payload )

        ExportDone ->
            ( { model | exporting = False }, Cmd.none )

        LogBrickClick brickId ->
            ( model
            , logBrick
                (E.object
                    [ ( "brickId", E.int brickId )
                    , ( "pieceId"
                      , model.pieces
                            |> List.filter (\p -> List.any (\br -> br.id == brickId) p.bricks)
                            |> List.head
                            |> Maybe.map (.id >> E.int)
                            |> Maybe.withDefault E.null
                      )
                    ]
                )
            )

        DragPieceStart pid ->
            ( { model | draggingPieceId = Just pid }, Cmd.none )

        DragPieceEnd ->
            ( { model | draggingPieceId = Nothing, dragOverWaveId = Nothing, dragInsertBeforeId = Nothing }, Cmd.none )

        DragEnterWave waveId ->
            ( { model | dragOverWaveId = Just waveId, dragInsertBeforeId = Nothing }, Cmd.none )

        DragEnterPiece pid ->
            ( { model | dragInsertBeforeId = Just pid }, Cmd.none )

        DropOnWave targetWaveId ->
            case model.draggingPieceId of
                Nothing ->
                    ( { model | dragOverWaveId = Nothing, dragInsertBeforeId = Nothing }, Cmd.none )

                Just pid ->
                    let
                        insertBefore =
                            model.dragInsertBeforeId

                        insertInto pids =
                            let
                                filtered =
                                    List.filter ((/=) pid) pids
                            in
                            case insertBefore of
                                Just beforeId ->
                                    List.concatMap
                                        (\p ->
                                            if p == beforeId then
                                                [ pid, p ]

                                            else
                                                [ p ]
                                        )
                                        filtered

                                Nothing ->
                                    filtered ++ [ pid ]

                        targetIsLocked =
                            case targetWaveId of
                                Just wid ->
                                    model.waves |> List.any (\wv -> wv.id == wid && wv.locked)

                                Nothing ->
                                    False

                        sourceIsLocked =
                            model.waves |> List.any (\wv -> List.member pid wv.pieceIds && wv.locked)

                        newWaves =
                            if targetIsLocked || sourceIsLocked then
                                model.waves

                            else
                                model.waves
                                    |> List.map (\wv -> { wv | pieceIds = List.filter ((/=) pid) wv.pieceIds })
                                    |> List.map
                                        (\wv ->
                                            case targetWaveId of
                                                Just wid ->
                                                    if wv.id == wid then
                                                        { wv | pieceIds = insertInto wv.pieceIds }

                                                    else
                                                        wv

                                                Nothing ->
                                                    wv
                                        )
                    in
                    ( { model | waves = newWaves, draggingPieceId = Nothing, dragOverWaveId = Nothing, dragInsertBeforeId = Nothing }, Cmd.none )

        ToggleWaveLock wid ->
            ( { model | waves = List.map (\w -> if w.id == wid then { w | locked = not w.locked } else w) model.waves }
            , Cmd.none
            )

        GotViewport viewport ->
            case model.loadState of
                Loaded response ->
                    let
                        vh =
                            viewport.viewport.height

                        -- Available height for SVG = canvas-area height (88% of body) minus padding
                        availableH =
                            (vh - 48) * 0.88 - 32

                        svgH =
                            response.canvas.height + 20

                        scale =
                            Basics.min 1.0 (availableH / svgH)
                    in
                    ( { model | svgScale = scale }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        NoOp ->
            ( model, Cmd.none )


-- ── Helpers ─────────────────────────────────────────────────────────────────


recalcPieceBbox : Dict Int Brick -> Piece -> Piece
recalcPieceBbox bricksById piece =
    let
        bricks =
            List.filterMap (\bid -> Dict.get bid bricksById) piece.brickIds

        newBrickRefs =
            List.map (\b -> BrickRef b.id b.x b.y b.width b.height) bricks

        xs =
            List.map .x bricks

        ys =
            List.map .y bricks

        x2s =
            List.map (\b -> b.x + b.width) bricks

        y2s =
            List.map (\b -> b.y + b.height) bricks
    in
    case List.minimum xs of
        Nothing ->
            piece

        Just x ->
            case ( List.minimum ys, List.maximum x2s, List.maximum y2s ) of
                ( Just y, Just x2, Just y2 ) ->
                    { piece | x = x, y = y, width = x2 - x, height = y2 - y, bricks = newBrickRefs, polygon = [] }

                _ ->
                    piece


editHasChanges : Model -> Bool
editHasChanges model =
    List.sort model.editBrickIds /= List.sort model.editOriginalBrickIds



-- ── HTTP ────────────────────────────────────────────────────────────────────


uploadTif : File -> Cmd Msg
uploadTif file =
    Http.post
        { url = "/api/upload_tif"
        , body = Http.multipartBody [ Http.filePart "file" file ]
        , expect = Http.expectJson GotUploadResponse (D.field "path" D.string)
        }


loadTif : String -> Cmd Msg
loadTif path =
    Http.post
        { url = "/api/load_tif"
        , body = Http.jsonBody (E.object [ ( "path", E.string path ) ])
        , expect = Http.expectJson GotLoadResponse decodeLoadResponse
        }


mergeBricks : Int -> Int -> Int -> Cmd Msg
mergeBricks targetCount minBorder seed =
    Http.post
        { url = "/api/merge"
        , body =
            Http.jsonBody
                (E.object
                    [ ( "target_count", E.int targetCount )
                    , ( "seed", E.int seed )
                    , ( "min_border", E.int minBorder )
                    ]
                )
        , expect = Http.expectJson GotMergeResponse decodeMergeResponse
        }



recomputePiecePolygons : List Piece -> Cmd Msg
recomputePiecePolygons pieces =
    Http.post
        { url = "/api/merge"
        , body =
            Http.jsonBody
                (E.object
                    [ ( "pieces"
                      , E.list
                            (\p ->
                                E.object
                                    [ ( "id", E.int p.id )
                                    , ( "brick_ids", E.list E.int p.brickIds )
                                    ]
                            )
                            pieces
                      )
                    ]
                )
        , expect = Http.expectJson GotPiecePolygons decodePiecePolygonResponse
        }


decodePiecePolygonResponse : D.Decoder (List ( Int, List Point ))
decodePiecePolygonResponse =
    D.field "pieces"
        (D.list
            (D.map2 Tuple.pair
                (D.field "id" D.int)
                (D.field "polygon" (D.list decodePoint))
            )
        )


-- ── Decoders ────────────────────────────────────────────────────────────────


decodeLoadResponse : D.Decoder LoadResponse
decodeLoadResponse =
    D.map4 LoadResponse
        (D.field "canvas" decodeCanvas)
        (D.field "bricks" (D.list decodeBrick))
        (D.field "has_composite" D.bool)
        (D.field "has_base" D.bool)


decodeCanvas : D.Decoder Canvas
decodeCanvas =
    D.map2 Canvas
        (D.field "width" D.float)
        (D.field "height" D.float)


decodeBrick : D.Decoder Brick
decodeBrick =
    D.map8 Brick
        (D.field "id" D.int)
        (D.field "x" D.float)
        (D.field "y" D.float)
        (D.field "width" D.float)
        (D.field "height" D.float)
        (D.field "type" D.string)
        (D.field "neighbors" (D.list D.int))
        (D.field "polygon" (D.list decodePoint))


decodePoint : D.Decoder Point
decodePoint =
    D.map2 Tuple.pair
        (D.index 0 D.float)
        (D.index 1 D.float)


decodeMergeResponse : D.Decoder MergeResponse
decodeMergeResponse =
    D.map MergeResponse
        (D.field "pieces" (D.list decodePiece))


decodePiece : D.Decoder Piece
decodePiece =
    D.map8 Piece
        (D.field "id" D.int)
        (D.field "x" D.float)
        (D.field "y" D.float)
        (D.field "width" D.float)
        (D.field "height" D.float)
        (D.field "brick_ids" (D.list D.int))
        (D.field "bricks" (D.list decodeBrickRef))
        (D.field "polygon" (D.list decodePoint))


decodeBrickRef : D.Decoder BrickRef
decodeBrickRef =
    D.map5 BrickRef
        (D.field "id" D.int)
        (D.field "x" D.float)
        (D.field "y" D.float)
        (D.field "width" D.float)
        (D.field "height" D.float)


decodePieceImages : D.Decoder (List ( Int, String ))
decodePieceImages =
    D.list
        (D.map2 Tuple.pair
            (D.field "id" D.int)
            (D.field "dataUrl" D.string)
        )



-- ── Encoders ────────────────────────────────────────────────────────────────


encodePieceList : List Piece -> E.Value
encodePieceList pieces =
    E.list encodePiece pieces


encodePiece : Piece -> E.Value
encodePiece piece =
    E.object
        [ ( "id", E.int piece.id )
        , ( "x", E.float piece.x )
        , ( "y", E.float piece.y )
        , ( "w", E.float piece.width )
        , ( "h", E.float piece.height )
        , ( "bricks", E.list encodeBrickRef piece.bricks )
        ]


encodeBrickRef : BrickRef -> E.Value
encodeBrickRef b =
    E.object
        [ ( "id", E.int b.id )
        , ( "x", E.float b.x )
        , ( "y", E.float b.y )
        , ( "w", E.float b.width )
        , ( "h", E.float b.height )
        ]


httpErrorToString : Http.Error -> String
httpErrorToString err =
    case err of
        Http.BadUrl url ->
            "Bad URL: " ++ url

        Http.Timeout ->
            "Request timed out"

        Http.NetworkError ->
            "Network error"

        Http.BadStatus code ->
            "Server error: " ++ String.fromInt code

        Http.BadBody m ->
            "Bad response: " ++ m



-- ── View ─────────────────────────────────────────────────────────────────────


view : Model -> Html Msg
view model =
    div [ class "app" ]
        [ viewTitleBar model
        , viewBody model
        ]


viewBody : Model -> Html Msg
viewBody model =
    if model.appMode == ModeInit then
        div [ class "app-body-empty", onClick PickFile ] []

    else
        case model.loadState of
            Loaded response ->
                div [ class "app-body" ]
                    [ viewCanvasCol model response
                    , div [ class "resize-handle" ] []
                    , viewToolsCol model response
                    , viewBodyOverlay model
                    ]

            _ ->
                div [ class "app-body" ]
                    [ div [ class "canvas-col" ]
                        [ div [ class "canvas-area" ]
                            [ div [ class "canvas-spinner-overlay" ] [ div [ class "canvas-spinner" ] [] ] ]
                        ]
                    , div [ class "tools-col" ] []
                    ]


viewBodyOverlay : Model -> Html Msg
viewBodyOverlay model =
    let
        msg =
            if model.generateState == Compositing then
                Just "Generating puzzle\u{2026}"

            else if model.recomputing then
                Just "Updating pieces\u{2026}"

            else if model.exporting then
                Just "Exporting\u{2026}"

            else
                Nothing
    in
    case msg of
        Nothing ->
            text ""

        Just label ->
            div [ class "body-overlay" ]
                [ div [ class "overlay-spinner" ] []
                , div [ class "overlay-label" ] [ text label ]
                ]


viewTitleBar : Model -> Html Msg
viewTitleBar model =
    let
        isLoaded =
            case model.loadState of
                Loaded _ ->
                    True

                _ ->
                    False

        isLoadingTif =
            model.loadState == Uploading || model.loadState == Loading

        isBusy =
            isLoadingTif || model.recomputing || model.exporting

        isGenerating =
            model.generateState == Compositing

        isGenerated =
            model.generateState == Generated

        hasFile =
            not (String.isEmpty model.selectedFileName)

        assignedIds =
            model.waves |> List.concatMap .pieceIds

        hasUnassigned =
            List.any (\p -> not (List.member p.id assignedIds)) model.pieces

        canExport =
            isGenerated && not isBusy && not isGenerating && not hasUnassigned
    in
    div [ class "title-bar" ]
        [ span [ class "app-title" ] [ text "House Puzzle" ]
        , span [ class "version-tag" ] [ text "v0.1" ]
        , div [ class "mode-buttons" ]
            [ button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModeInit )
                    , ( "loading", isLoadingTif )
                    ]
                , disabled (isBusy || isGenerating)
                , onClick PickFile
                ]
                [ text
                    (if isLoadingTif then
                        "Loading\u{2026}"

                     else if hasFile then
                        "Reset"

                     else
                        "Start"
                    )
                ]
            , span [ class "mode-sep" ] [ text "\u{2192}" ]
            , button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModeTif )
                    , ( "loading", isGenerating )
                    ]
                , disabled (not isLoaded || isBusy || isGenerating)
                , onClick (SetAppMode ModeTif)
                ]
                [ text
                    (if isGenerating then
                        "Generating\u{2026}"

                     else
                        "Generate"
                    )
                ]
            , span [ class "mode-sep" ] [ text "\u{2192}" ]
            , button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModePieces )
                    , ( "loading", model.recomputing && model.appMode == ModePieces )
                    ]
                , disabled (not isGenerated || isBusy || isGenerating)
                , onClick (SetAppMode ModePieces)
                ]
                [ text "Pieces" ]
            , span [ class "mode-sep" ] [ text "\u{21C4}" ]
            , button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModeBlueprint )
                    ]
                , disabled (not isGenerated || isBusy || isGenerating)
                , onClick (SetAppMode ModeBlueprint)
                ]
                [ text "Blueprint" ]
            , span [ class "mode-sep" ] [ text "\u{21C4}" ]
            , button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModeWaves )
                    ]
                , disabled (not isGenerated || isBusy || isGenerating)
                , onClick (SetAppMode ModeWaves)
                ]
                [ text "Waves" ]
            , span [ class "mode-sep" ] [ text "\u{2192}" ]
            , button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "export-btn", True )
                    , ( "active", model.appMode == ModeExport )
                    , ( "loading", model.exporting )
                    ]
                , disabled (not canExport)
                , onClick (SetAppMode ModeExport)
                , title
                    (if hasUnassigned && isGenerated then
                        "All pieces must be assigned to waves before exporting"

                     else
                        ""
                    )
                ]
                [ text "Export" ]
            ]
        ]


viewCanvasCol : Model -> LoadResponse -> Html Msg
viewCanvasCol model response =
    div [ class "canvas-col" ]
        ([ div [ class "canvas-area" ]
            [ viewMainSvg response model
            , if model.recomputing then
                div [ class "canvas-spinner-overlay" ] [ div [ class "canvas-spinner" ] [] ]

              else
                text ""
            ]
         ]
            ++ (if model.appMode == ModeWaves then
                    [ viewWaveTray model response ]

                else
                    []
               )
        )


viewWaveTray : Model -> LoadResponse -> Html Msg
viewWaveTray model _ =
    let
        activeWaveId =
            model.selectedWaveId

        activeWave =
            model.waves |> List.filter (\w -> Just w.id == activeWaveId) |> List.head

        activeWavePieceIds =
            activeWave |> Maybe.map .pieceIds |> Maybe.withDefault []

        isLocked =
            activeWave |> Maybe.map .locked |> Maybe.withDefault False
    in
    div
        [ classList
            [ ( "wave-tray", True )
            , ( "drag-over", not isLocked && model.dragOverWaveId == Just activeWaveId )
            ]
        , preventDefaultOn "dragover" (D.succeed ( NoOp, True ))
        , on "dragenter" (D.succeed (DragEnterWave activeWaveId))
        , on "drop" (D.succeed (DropOnWave activeWaveId))
        ]
        [ div [ class "wave-tray-scroll" ]
            (List.concatMap
                (\pid ->
                    let
                        showMarker =
                            not isLocked && model.draggingPieceId /= Nothing && model.dragInsertBeforeId == Just pid

                        marker =
                            if showMarker then
                                [ div [ class "drag-insert-marker-v" ] [] ]

                            else
                                []

                        thumb =
                            case ( model.pieces |> List.filter (\p -> p.id == pid) |> List.head, Dict.get pid model.pieceImages ) of
                                ( Just piece, Just dataUrl ) ->
                                    [ viewWaveTrayThumb piece isLocked model.svgScale model.hoveredPieceId dataUrl ]

                                _ ->
                                    []
                    in
                    marker ++ thumb
                )
                activeWavePieceIds
                ++ (if not isLocked && model.draggingPieceId /= Nothing && model.dragInsertBeforeId == Nothing && model.dragOverWaveId == Just activeWaveId then
                        [ div [ class "drag-insert-marker-v" ] [] ]

                    else
                        []
                   )
            )
        ]


-- scale: computed from viewport height and SVG natural height (stored in model.svgScale).
-- Produces exact px dimensions matching how the piece appears in the house view.
viewWaveTrayThumb : Piece -> Bool -> Float -> Maybe Int -> String -> Html Msg
viewWaveTrayThumb piece isLocked scale hoveredId dataUrl =
    let
        isHovered =
            hoveredId == Just piece.id

        widthCss =
            String.fromFloat (piece.width * scale) ++ "px"

        dragAttrs =
            if isLocked then
                []

            else
                [ attribute "draggable" "true"
                , on "dragstart" (D.succeed (DragPieceStart piece.id))
                , on "dragend" (D.succeed DragPieceEnd)
                , stopPropagationOn "dragenter" (D.succeed ( DragEnterPiece piece.id, True ))
                ]
    in
    div
        ([ classList [ ( "wave-tray-thumb", True ), ( "hovered", isHovered ) ]
         , style "width" widthCss
         , style "aspect-ratio" (String.fromFloat (piece.width / piece.height))
         , onMouseEnter (SetHoveredPiece (Just piece.id))
         , onMouseLeave (SetHoveredPiece Nothing)
         ]
            ++ dragAttrs
        )
        [ img [ src dataUrl ] []
        ]


viewToolsCol : Model -> LoadResponse -> Html Msg
viewToolsCol model response =
    div [ class "tools-col" ]
        [ case model.appMode of
            ModeInit ->
                text ""

            ModeTif ->
                viewTifTools model response

            ModePieces ->
                viewPiecesTools model

            ModeBlueprint ->
                viewBlueprintTools model

            ModeWaves ->
                viewWavesTools model

            ModeExport ->
                viewExportTools model
        ]


viewTifTools : Model -> LoadResponse -> Html Msg
viewTifTools model _ =
    let
        isLoaded =
            case model.loadState of
                Loaded _ -> True
                _ -> False

        isBusy =
            model.loadState == Uploading || model.loadState == Loading || model.recomputing || model.exporting

        isGenerating =
            model.generateState == Compositing
    in
    div [ class "tools-pane" ]
        [ viewStatusBadge model
        , h2 [] [ text "Puzzle Parameters" ]
        , div [ class "param-group" ]
            [ label [] [ text "Target Pieces ", span [ class "value" ] [ text (String.fromInt model.targetCount) ] ]
            , input [ type_ "range", Html.Attributes.min "5", Html.Attributes.max "181", value (String.fromInt model.targetCount), onInput SetTargetCount ] []
            ]
        , div [ class "param-group" ]
            [ label [] [ text "Min Border ", span [ class "value" ] [ text (String.fromInt model.minBorder) ], text "px" ]
            , input [ type_ "range", Html.Attributes.min "0", Html.Attributes.max "50", value (String.fromInt model.minBorder), onInput SetMinBorder ] []
            ]
        , div [ class "param-group" ]
            [ label [] [ text "Seed ", span [ class "value" ] [ text (String.fromInt model.seed) ] ]
            , input [ type_ "number", value (String.fromInt model.seed), onInput SetSeed, Html.Attributes.min "0", Html.Attributes.max "99999" ] []
            ]
        , h2 [] [ text "Stats" ]
        , viewStats model
        , div [ class "tools-divider" ] []
        , button
            [ class "primary"
            , disabled (not isLoaded || isBusy || isGenerating)
            , onClick RequestGenerate
            ]
            [ text
                (if isGenerating then
                    "Generating\u{2026}"

                 else
                    "Generate Puzzle"
                )
            ]
        ]


viewPiecesTools : Model -> Html Msg
viewPiecesTools model =
    div [ class "tools-pane" ]
        (if model.editMode then
            viewEditControls model

         else
            [ div [ class "checkbox-group" ]
                [ input [ type_ "checkbox", id "showOutlines", checked model.showOutlines, onCheck ToggleOutlines ] []
                , label [ for "showOutlines" ] [ text "Show piece outlines" ]
                ]
            , div [ class "checkbox-group" ]
                [ input [ type_ "checkbox", id "showGrid", checked model.showGrid, onCheck ToggleGrid ] []
                , label [ for "showGrid" ] [ text "Show grid" ]
                ]
            , div [ class "tools-divider" ] []
            , case model.selectedPieceId of
                Just pid ->
                    div [ class "piece-info" ]
                        [ div [ class "piece-info-label" ] [ text ("Piece #" ++ String.fromInt pid) ]
                        , button
                            [ class "primary"
                            , onClick StartEdit
                            , disabled model.recomputing
                            ]
                            [ text ("Edit Piece #" ++ String.fromInt pid) ]
                        ]

                Nothing ->
                    div [ class "piece-info-empty" ] [ text "Click a piece to select" ]
            ]
        )


viewBlueprintTools : Model -> Html Msg
viewBlueprintTools model =
    div [ class "tools-pane" ]
        [ div [ class "checkbox-group" ]
            [ input [ type_ "checkbox", id "showGrid", checked model.showGrid, onCheck ToggleGrid ] []
            , label [ for "showGrid" ] [ text "Show grid" ]
            ]
        ]


viewWavesTools : Model -> Html Msg
viewWavesTools model =
    let
        assignedIds =
            List.concatMap .pieceIds model.waves

        totalPieces =
            List.length model.pieces

        assignedCount =
            List.length assignedIds

        unassignedPieces =
            List.filter (\p -> not (List.member p.id assignedIds)) model.pieces
    in
    div [ class "tools-pane waves-tools" ]
        [ div [ class "waves-header" ]
            [ h2 [] [ text "Waves" ]
            , span [ class "wave-count" ]
                [ text
                    (if totalPieces > 0 then
                        String.fromInt assignedCount ++ "/" ++ String.fromInt totalPieces

                     else
                        ""
                    )
                ]
            ]
        , div [ class "wave-toolbar" ]
            [ button [ onClick AddWave ] [ text "New wave" ] ]
        , div [ class "waves-body" ]
            (List.map (viewWaveRow model model.waves) model.waves
                ++ [ viewUnassignedRow model unassignedPieces ]
            )
        ]


viewExportTools : Model -> Html Msg
viewExportTools model =
    let
        assignedIds =
            model.waves |> List.concatMap .pieceIds

        hasUnassigned =
            List.any (\p -> not (List.member p.id assignedIds)) model.pieces
    in
    div [ class "tools-pane" ]
        [ button
            [ class "primary"
            , onClick RequestExport
            , disabled (hasUnassigned || model.exporting)
            , title
                (if hasUnassigned then
                    "All pieces must be assigned to waves before exporting"

                 else
                    ""
                )
            ]
            [ text
                (if model.exporting then
                    "Exporting\u{2026}"

                 else
                    "Export ZIP"
                )
            ]
        ]


viewMainSvg : LoadResponse -> Model -> Html Msg
viewMainSvg response model =
    let
        cw =
            response.canvas.width

        ch =
            response.canvas.height

        w =
            String.fromFloat cw

        h =
            String.fromFloat ch

        isGenerated =
            model.generateState == Generated

        isPieces =
            model.appMode /= ModeBlueprint

        showPieceImages =
            (model.appMode == ModePieces || model.appMode == ModeWaves) && isGenerated && not (Dict.isEmpty model.pieceImages)

        showComposite =
            isPieces && not isGenerated && response.hasComposite

        -- Pieces hidden by invisible waves
        hiddenPieceIds =
            model.waves
                |> List.filter (\wv -> not wv.visible)
                |> List.concatMap .pieceIds

        visiblePieces =
            List.filter (\p -> not (List.member p.id hiddenPieceIds)) model.pieces

        -- Blueprint layer: always shown post-gen (underneath everything) so hidden-wave gaps show piece outlines
        blueprintLayer =
            if (not model.editMode) && isGenerated then
                List.map viewPieceBlueprintPath model.pieces

            else
                []

        -- Base layer (on top of blueprint)
        baseLayer =
            if model.editMode then
                if response.hasComposite then
                    [ Svg.image
                        [ SA.x "0"
                        , SA.y "0"
                        , SA.width w
                        , SA.height h
                        , attribute "href" "/api/composite.png"
                        ]
                        []
                    ]

                else
                    []

            else if showPieceImages then
                List.map (viewPieceImage model.pieceImages) visiblePieces

            else if showComposite then
                [ Svg.image
                    [ SA.x "0"
                    , SA.y "0"
                    , SA.width w
                    , SA.height h
                    , attribute "href" "/api/composite.png"
                    ]
                    []
                ]

            else if isGenerated then
                -- Blueprint mode after generate: hide bricks so piece polygons show through
                []

            else
                List.map viewBrickPath response.bricks

        -- Composite brick hover overlays (pre-gen only)
        compositeOverlays =
            if showComposite then
                List.map viewBrickOverlay response.bricks

            else
                []

        -- Edit mode: brick overlays for toggling
        editOverlays =
            if model.editMode then
                List.map (viewBrickEditOverlay model.editBrickIds) response.bricks

            else
                []

        -- Grid lines
        gridLayer =
            if (not model.editMode) && model.showGrid then
                viewGrid cw ch (model.appMode == ModeBlueprint)

            else
                []

        -- Piece outlines (post-gen, pieces/waves mode only, not in edit)
        outlineLayer =
            if (not model.editMode) && isGenerated && model.showOutlines && (model.appMode == ModePieces || model.appMode == ModeWaves) then
                List.map viewPieceOutline visiblePieces

            else
                []

        -- Piece interaction overlays (post-gen, not in edit)
        effectiveHoverId =
            if model.draggingPieceId /= Nothing then
                model.draggingPieceId

            else
                model.hoveredPieceId

        pieceOverlays =
            if (not model.editMode) && isGenerated then
                List.map (viewPieceOverlay model.appMode effectiveHoverId model.selectedPieceId model.selectedWaveId model.waves) visiblePieces

            else
                []
    in
    Svg.svg
        [ SA.viewBox ("-10 -10 " ++ String.fromFloat (cw + 20) ++ " " ++ String.fromFloat (ch + 20))
        , SA.class "house-svg"
        , SA.width (String.fromFloat (cw + 20))
        , SA.height (String.fromFloat (ch + 20))
        ]
        (if model.editMode then
            [ Svg.g [] baseLayer
            , Svg.g [] editOverlays
            ]

         else
            [ Svg.g [] blueprintLayer
            , Svg.g [] baseLayer
            , Svg.g [] compositeOverlays
            , Svg.g [] outlineLayer
            , Svg.g [] gridLayer
            , Svg.g [] pieceOverlays
            ]
        )


viewPieceImage : Dict Int String -> Piece -> Svg.Svg Msg
viewPieceImage images piece =
    case Dict.get piece.id images of
        Just dataUrl ->
            Svg.image
                [ SA.x (String.fromFloat piece.x)
                , SA.y (String.fromFloat piece.y)
                , SA.width (String.fromFloat piece.width)
                , SA.height (String.fromFloat piece.height)
                , attribute "href" dataUrl
                ]
                []

        Nothing ->
            Svg.rect
                [ SA.x (String.fromFloat piece.x)
                , SA.y (String.fromFloat piece.y)
                , SA.width (String.fromFloat piece.width)
                , SA.height (String.fromFloat piece.height)
                , SA.fill "rgba(255,100,50,0.2)"
                , SA.stroke "#f64"
                , SA.strokeWidth "1"
                ]
                []


viewBrickPath : Brick -> Svg.Svg Msg
viewBrickPath brick =
    let
        absPoints =
            List.map (\( x, y ) -> ( x + brick.x, y + brick.y )) brick.polygon

        pointsAttr =
            absPoints
                |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                |> String.join " "
    in
    if List.isEmpty absPoints then
        Svg.rect
            [ SA.x (String.fromFloat brick.x)
            , SA.y (String.fromFloat brick.y)
            , SA.width (String.fromFloat brick.width)
            , SA.height (String.fromFloat brick.height)
            , SA.fill "#2a5da8"
            , SA.stroke "white"
            , SA.strokeWidth "4"
            , attribute "vector-effect" "non-scaling-stroke"
            ]
            []

    else
        Svg.polygon
            [ SA.points pointsAttr
            , SA.fill "#2a5da8"
            , SA.stroke "white"
            , SA.strokeWidth "4"
            , SA.strokeLinejoin "round"
            , attribute "stroke-linecap" "round"
            , attribute "paint-order" "fill stroke"
            , attribute "vector-effect" "non-scaling-stroke"
            , attribute "data-brick-id" (String.fromInt brick.id)
            , SA.class "brick-path"
            ]
            []


viewBrickOverlay : Brick -> Svg.Svg Msg
viewBrickOverlay brick =
    let
        absPoints =
            List.map (\( x, y ) -> ( x + brick.x, y + brick.y )) brick.polygon

        pointsAttr =
            absPoints
                |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                |> String.join " "
    in
    if List.isEmpty absPoints then
        Svg.rect
            [ SA.x (String.fromFloat brick.x)
            , SA.y (String.fromFloat brick.y)
            , SA.width (String.fromFloat brick.width)
            , SA.height (String.fromFloat brick.height)
            , SA.fill "transparent"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "brick-overlay"
            , onClick (LogBrickClick brick.id)
            ]
            []

    else
        Svg.polygon
            [ SA.points pointsAttr
            , SA.fill "transparent"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "brick-overlay"
            , onClick (LogBrickClick brick.id)
            ]
            []


viewBrickEditOverlay : List Int -> Brick -> Svg.Svg Msg
viewBrickEditOverlay editBrickIds brick =
    let
        inEdit =
            List.member brick.id editBrickIds

        absPoints =
            List.map (\( x, y ) -> ( x + brick.x, y + brick.y )) brick.polygon

        pointsAttr =
            absPoints
                |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                |> String.join " "

        cls =
            if inEdit then
                "brick-edit-in"

            else
                "brick-edit-out"
    in
    if List.isEmpty absPoints then
        Svg.rect
            [ SA.x (String.fromFloat brick.x)
            , SA.y (String.fromFloat brick.y)
            , SA.width (String.fromFloat brick.width)
            , SA.height (String.fromFloat brick.height)
            , SA.class cls
            , attribute "vector-effect" "non-scaling-stroke"
            , onClick (ToggleBrickInEdit brick.id)
            ]
            []

    else
        Svg.polygon
            [ SA.points pointsAttr
            , SA.class cls
            , attribute "vector-effect" "non-scaling-stroke"
            , onClick (ToggleBrickInEdit brick.id)
            ]
            []


viewPieceBlueprintPath : Piece -> Svg.Svg Msg
viewPieceBlueprintPath piece =
    if List.isEmpty piece.polygon then
        Svg.g [] []

    else
        let
            pointsAttr =
                piece.polygon
                    |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                    |> String.join " "
        in
        Svg.polygon
            [ SA.points pointsAttr
            , SA.fill "#2a5da8"
            , SA.stroke "white"
            , SA.strokeWidth "4"
            , SA.strokeLinejoin "round"
            , attribute "stroke-linecap" "round"
            , attribute "paint-order" "fill stroke"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "brick-path"
            ]
            []


viewPieceOutline : Piece -> Svg.Svg Msg
viewPieceOutline piece =
    if List.isEmpty piece.polygon then
        Svg.g [] []

    else
        let
            pointsAttr =
                piece.polygon
                    |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                    |> String.join " "
        in
        Svg.polygon
            [ SA.points pointsAttr
            , SA.fill "transparent"
            , SA.stroke "#555"
            , SA.strokeWidth "1"
            , SA.strokeLinejoin "round"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "piece-outline"
            ]
            []


waveColorClass : Int -> String
waveColorClass idx =
    "wc-" ++ String.fromInt (modBy 7 idx)


viewPieceOverlay : AppMode -> Maybe Int -> Maybe Int -> Maybe Int -> List Wave -> Piece -> Svg.Svg Msg
viewPieceOverlay appMode hoveredId selectedId selectedWaveId waves piece =
    let
        inAssignMode =
            selectedWaveId /= Nothing

        isHov =
            hoveredId == Just piece.id

        isSel =
            not inAssignMode && selectedId == Just piece.id

        maybePieceWaveClass =
            waves
                |> List.indexedMap Tuple.pair
                |> List.filter (\( _, w ) -> w.visible && List.member piece.id w.pieceIds)
                |> List.head
                |> Maybe.map (\( idx, _ ) -> waveColorClass idx)

        clsStr =
            [ "piece-overlay"
            , Maybe.withDefault "" maybePieceWaveClass
            , if isHov then "hovered" else ""
            , if isSel then "selected" else ""
            ]
                |> List.filter ((/=) "")
                |> String.join " "

        clickMsg =
            if inAssignMode then
                AssignPieceToWave piece.id

            else if appMode == ModePieces then
                SelectAndEdit piece.id

            else
                SelectPiece piece.id
    in
    if List.isEmpty piece.polygon then
        Svg.g [] []

    else
        let
            pointsAttr =
                piece.polygon
                    |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                    |> String.join " "
        in
        Svg.polygon
            [ SA.points pointsAttr
            , SA.class clsStr
            , onClick clickMsg
            , onMouseEnter (SetHoveredPiece (Just piece.id))
            , onMouseLeave (SetHoveredPiece Nothing)
            ]
            []


viewGrid : Float -> Float -> Bool -> List (Svg.Svg Msg)
viewGrid cw ch isBlueprint =
    let
        gridStep =
            211.0

        color =
            if isBlueprint then
                "#ff0000"

            else
                "#e0a050"

        numV =
            floor (cw / gridStep)

        numH =
            floor (ch / gridStep)

        vLines =
            List.map
                (\i ->
                    let
                        x =
                            toFloat i * gridStep
                    in
                    Svg.line
                        [ SA.x1 (String.fromFloat x)
                        , SA.y1 "0"
                        , SA.x2 (String.fromFloat x)
                        , SA.y2 (String.fromFloat ch)
                        , SA.stroke color
                        , SA.strokeWidth "1"
                        , attribute "vector-effect" "non-scaling-stroke"
                        ]
                        []
                )
                (List.range 1 numV)

        hLines =
            List.map
                (\i ->
                    let
                        y =
                            ch - toFloat i * gridStep
                    in
                    Svg.line
                        [ SA.x1 "0"
                        , SA.y1 (String.fromFloat y)
                        , SA.x2 (String.fromFloat cw)
                        , SA.y2 (String.fromFloat y)
                        , SA.stroke color
                        , SA.strokeWidth "1"
                        , attribute "vector-effect" "non-scaling-stroke"
                        ]
                        []
                )
                (List.range 1 numH)
    in
    vLines ++ hLines


iconEye : Html msg
iconEye =
    Svg.svg [ SA.viewBox "0 0 24 24", SA.width "14", SA.height "14", SA.fill "currentColor" ]
        [ Svg.path [ SA.d "M23.271,9.419C21.72,6.893,18.192,2.655,12,2.655S2.28,6.893.729,9.419a4.908,4.908,0,0,0,0,5.162C2.28,17.107,5.808,21.345,12,21.345s9.72-4.238,11.271-6.764A4.908,4.908,0,0,0,23.271,9.419Zm-1.705,4.115C20.234,15.7,17.219,19.345,12,19.345S3.766,15.7,2.434,13.534a2.918,2.918,0,0,1,0-3.068C3.766,8.3,6.781,4.655,12,4.655s8.234,3.641,9.566,5.811A2.918,2.918,0,0,1,21.566,13.534Z" ] []
        , Svg.path [ SA.d "M12,7a5,5,0,1,0,5,5A5.006,5.006,0,0,0,12,7Zm0,8a3,3,0,1,1,3-3A3,3,0,0,1,12,15Z" ] []
        ]


iconEyeCrossed : Html msg
iconEyeCrossed =
    Svg.svg [ SA.viewBox "0 0 24 24", SA.width "14", SA.height "14", SA.fill "currentColor" ]
        [ Svg.path [ SA.d "M23.271,9.419A15.866,15.866,0,0,0,19.9,5.51l2.8-2.8a1,1,0,0,0-1.414-1.414L18.241,4.345A12.054,12.054,0,0,0,12,2.655C5.809,2.655,2.281,6.893.729,9.419a4.908,4.908,0,0,0,0,5.162A15.866,15.866,0,0,0,4.1,18.49l-2.8,2.8a1,1,0,1,0,1.414,1.414l3.052-3.052A12.054,12.054,0,0,0,12,21.345c6.191,0,9.719-4.238,11.271-6.764A4.908,4.908,0,0,0,23.271,9.419ZM2.433,13.534a2.918,2.918,0,0,1,0-3.068C3.767,8.3,6.782,4.655,12,4.655A10.1,10.1,0,0,1,16.766,5.82L14.753,7.833a4.992,4.992,0,0,0-6.92,6.92l-2.31,2.31A13.723,13.723,0,0,1,2.433,13.534ZM15,12a3,3,0,0,1-3,3,2.951,2.951,0,0,1-1.285-.3L14.7,10.715A2.951,2.951,0,0,1,15,12ZM9,12a3,3,0,0,1,3-3,2.951,2.951,0,0,1,1.285.3L9.3,13.285A2.951,2.951,0,0,1,9,12Zm12.567,1.534C20.233,15.7,17.218,19.345,12,19.345A10.1,10.1,0,0,1,7.234,18.18l2.013-2.013a4.992,4.992,0,0,0,6.92-6.92l2.31-2.31a13.723,13.723,0,0,1,3.09,3.529A2.918,2.918,0,0,1,21.567,13.534Z" ] []
        ]


iconLock : Html msg
iconLock =
    Svg.svg [ SA.viewBox "0 0 24 24", SA.width "14", SA.height "14", SA.fill "currentColor" ]
        [ Svg.path [ SA.d "M19,8V7A7,7,0,0,0,5,7V8H2V21a3,3,0,0,0,3,3H19a3,3,0,0,0,3-3V8ZM7,7A5,5,0,0,1,17,7V8H7ZM20,21a1,1,0,0,1-1,1H5a1,1,0,0,1-1-1V10H20Z" ] []
        , Svg.rect [ SA.x "11", SA.y "14", SA.width "2", SA.height "4" ] []
        ]


viewWaveRow : Model -> List Wave -> Wave -> Html Msg
viewWaveRow model allWaves wave =
    let
        isSelected =
            model.selectedWaveId == Just wave.id

        waveIdx =
            allWaves
                |> List.indexedMap Tuple.pair
                |> List.filter (\( _, wv ) -> wv.id == wave.id)
                |> List.head
                |> Maybe.map Tuple.first
                |> Maybe.withDefault 0

        waveCount =
            List.length allWaves
    in
    div
        [ classList
            [ ( "wave-row", True )
            , ( waveColorClass waveIdx, True )
            , ( "selected", isSelected )
            , ( "locked", wave.locked )
            , ( "drag-over", not wave.locked && model.dragOverWaveId == Just (Just wave.id) )
            ]
        , preventDefaultOn "dragover" (D.succeed ( NoOp, True ))
        , on "dragenter" (D.succeed (DragEnterWave (Just wave.id)))
        , on "drop" (D.succeed (DropOnWave (Just wave.id)))
        ]
        [ div
            [ class "wave-row-header"
            , onClick
                (if isSelected && waveCount > 1 then
                    SelectWave Nothing

                 else
                    SelectWave (Just wave.id)
                )
            ]
            [ span
                [ classList [ ( "wave-eye", True ), ( "hidden", not wave.visible ) ]
                , stopPropagationOn "click" (D.succeed ( ToggleWaveVisibility wave.id, True ))
                , title (if wave.visible then "Hide wave" else "Show wave")
                ]
                [ if wave.visible then iconEye else iconEyeCrossed ]
            , span
                [ classList [ ( "wave-lock", True ), ( "locked", wave.locked ) ]
                , stopPropagationOn "click" (D.succeed ( ToggleWaveLock wave.id, True ))
                , title
                    (if wave.locked then
                        "Unlock wave"

                     else
                        "Lock wave"
                    )
                ]
                [ iconLock ]
            , span [ class "wave-piece-count-label" ]
                [ text (String.fromInt (List.length wave.pieceIds) ++ " pcs") ]
            , span [ class "wave-name-label" ]
                [ text wave.name ]
            , span [ class "wave-row-spacer" ] []
            , span [ class "wave-actions" ]
                [ button
                    [ stopPropagationOn "click" (D.succeed ( MoveWave wave.id -1, True ))
                    , disabled (waveIdx == 0)
                    , title "Move up"
                    ]
                    [ text "\u{25B2}" ]
                , button
                    [ stopPropagationOn "click" (D.succeed ( MoveWave wave.id 1, True ))
                    , disabled (waveIdx >= waveCount - 1)
                    , title "Move down"
                    ]
                    [ text "\u{25BC}" ]
                , button
                    [ stopPropagationOn "click" (D.succeed ( RemoveWave wave.id, True ))
                    , disabled (wave.locked || waveCount <= 1)
                    , title "Delete wave"
                    ]
                    [ text "\u{2715}" ]
                ]
            ]
        , div [ class "wave-pieces" ]
            (List.concatMap
                (\pid ->
                    let
                        showMarker =
                            not wave.locked && model.draggingPieceId /= Nothing && model.dragInsertBeforeId == Just pid

                        marker =
                            if showMarker then
                                [ div [ class "drag-insert-marker" ] [] ]

                            else
                                []

                        thumb =
                            Dict.get pid model.pieceImages
                                |> Maybe.map (viewPieceThumb (Just ( wave.id, pid )) wave.locked model.hoveredPieceId pid)
                                |> Maybe.map List.singleton
                                |> Maybe.withDefault []
                    in
                    marker ++ thumb
                )
                wave.pieceIds
                ++ (if not wave.locked && model.draggingPieceId /= Nothing && model.dragInsertBeforeId == Nothing && model.dragOverWaveId == Just (Just wave.id) then
                        [ div [ class "drag-insert-marker" ] [] ]

                    else
                        []
                   )
            )
        ]


viewUnassignedRow : Model -> List Piece -> Html Msg
viewUnassignedRow model unassignedPieces =
    if List.isEmpty model.pieces then
        text ""

    else
        div
            [ classList
                [ ( "wave-row", True )
                , ( "drag-over", model.dragOverWaveId == Just Nothing )
                ]
            , preventDefaultOn "dragover" (D.succeed ( NoOp, True ))
            , on "dragenter" (D.succeed (DragEnterWave Nothing))
            , on "drop" (D.succeed (DropOnWave Nothing))
            ]
            [ div [ class "wave-row-header" ]
                [ span [ class "wave-label unassigned-label" ] [ text "Unassigned" ]
                , span [ class "wave-piece-count" ]
                    [ text (String.fromInt (List.length unassignedPieces) ++ " pcs") ]
                ]
            , div [ class "wave-pieces" ]
                (List.filterMap
                    (\p ->
                        Dict.get p.id model.pieceImages
                            |> Maybe.map (viewPieceThumb Nothing False model.hoveredPieceId p.id)
                    )
                    unassignedPieces
                )
            ]


viewPieceThumb : Maybe ( Int, Int ) -> Bool -> Maybe Int -> Int -> String -> Html Msg
viewPieceThumb removeInfo isLocked hoveredId pieceId dataUrl =
    let
        isHovered =
            hoveredId == Just pieceId

        dragAttrs =
            if isLocked then
                []

            else
                [ attribute "draggable" "true"
                , on "dragstart" (D.succeed (DragPieceStart pieceId))
                , on "dragend" (D.succeed DragPieceEnd)
                , stopPropagationOn "dragenter" (D.succeed ( DragEnterPiece pieceId, True ))
                ]
    in
    div
        ([ classList [ ( "piece-thumb", True ), ( "hovered", isHovered ) ]
         , onMouseEnter (SetHoveredPiece (Just pieceId))
         , onMouseLeave (SetHoveredPiece Nothing)
         ]
            ++ dragAttrs
        )
        ([ img
            [ src dataUrl
            , style "max-height" "48px"
            , style "max-width" "80px"
            , style "display" "block"
            ]
            []
         , div [ class "piece-thumb-label" ] [ text ("#" ++ String.fromInt pieceId) ]
         ]
            ++ (case removeInfo of
                    Just ( wid, pid ) ->
                        [ button
                            [ class "piece-thumb-remove"
                            , onClick (RemovePieceFromWave wid pid)
                            , disabled isLocked
                            , title "Remove from wave"
                            ]
                            [ text "\u{2715}" ]
                        ]

                    Nothing ->
                        []
               )
        )


viewEditControls : Model -> List (Html Msg)
viewEditControls model =
    let
        changed =
            editHasChanges model

        pieceLabel =
            case model.selectedPieceId of
                Just pid ->
                    "Piece #" ++ String.fromInt pid

                Nothing ->
                    "Piece"

        brickCount =
            List.length model.editBrickIds
    in
    [ h2 [] [ text ("Editing " ++ pieceLabel) ]
    , div
        [ style "font-size" "11px"
        , style "color" "#aaa"
        , style "margin-bottom" "10px"
        , style "line-height" "1.5"
        ]
        [ text "Click bricks to add/remove."
        , br [] []
        , text (String.fromInt brickCount ++ " brick" ++ (if brickCount == 1 then "" else "s") ++ " selected.")
        ]
    , div [ class "btn-row" ]
        [ button
            [ class "primary"
            , onClick SaveEdit
            , disabled (not changed)
            ]
            [ text "Save" ]
        , button
            [ onClick CancelEdit ]
            [ text "Cancel" ]
        ]
    ]


viewStatusBadge : Model -> Html Msg
viewStatusBadge model =
    case model.loadState of
        Idle ->
            text ""

        Uploading ->
            span [ class "status loading" ] [ text "Uploading\u{2026}" ]

        Loading ->
            span [ class "status loading" ] [ text "Parsing TIF\u{2026}" ]

        Loaded _ ->
            text ""

        LoadError err ->
            span [ class "status error" ] [ text ("Error: " ++ err) ]


viewStats : Model -> Html Msg
viewStats model =
    let
        canvasInfo =
            case model.loadState of
                Loaded r ->
                    String.fromFloat r.canvas.width ++ "\u{00D7}" ++ String.fromFloat r.canvas.height

                _ ->
                    "-"

        brickCount =
            case model.loadState of
                Loaded r ->
                    String.fromInt (List.length r.bricks)

                _ ->
                    "-"

        pieceCount =
            if model.generateState == Generated then
                String.fromInt (List.length model.pieces)

            else
                "-"
    in
    div [ class "stats" ]
        [ div [ class "row" ]
            [ span [] [ text "Canvas" ]
            , span [ class "val" ] [ text canvasInfo ]
            ]
        , div [ class "row" ]
            [ span [] [ text "Total Bricks" ]
            , span [ class "val" ] [ text brickCount ]
            ]
        , div [ class "row" ]
            [ span [] [ text "Puzzle Pieces" ]
            , span [ class "val" ] [ text pieceCount ]
            ]
        ]



-- ── Subscriptions ────────────────────────────────────────────────────────────


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.batch
        [ gotPieceImages GotPieceImages
        , gotExportDone (\_ -> ExportDone)
        ]



-- ── Main ─────────────────────────────────────────────────────────────────────


main : Program () Model Msg
main =
    Browser.element
        { init = init
        , update = update
        , view = view
        , subscriptions = subscriptions
        }
