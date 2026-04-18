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
import Browser.Events
import Process
import Task



-- ── Types ──────────────────────────────────────────────────────────────────


type alias Point =
    ( Float, Float )


type alias Brick =
    { id : String
    , x : Float
    , y : Float
    , width : Float
    , height : Float
    , brickType : String
    , neighbors : List String
    , polygon : List Point
    , layerName : String
    }


type alias BrickRef =
    { id : String
    , x : Float
    , y : Float
    , width : Float
    , height : Float
    }


type alias Piece =
    { id : String
    , x : Float
    , y : Float
    , width : Float
    , height : Float
    , brickIds : List String
    , bricks : List BrickRef
    , polygon : List Point
    , imgUrl : String
    , outlineUrl : String
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
    , renderDpi : Float
    , warnings : List String
    , outlinesUrl : String
    , compositeUrl : String
    , blueprintBgUrl : Maybe String
    , lightsUrl : Maybe String
    , houseUnitsHigh : Float
    , key : String
    }


type alias MergeResponse =
    { pieces : List Piece
    }


type ColorPickTarget
    = WaveColorTarget Int
    | GroupColorTarget Int
    | GridColorTarget
    | OutlineColorTarget


type alias Wave =
    { id : Int
    , name : String
    , visible : Bool
    , locked : Bool
    , pieceIds : List String
    , hue : Float
    , opacity : Float
    }


type alias Group =
    { id : Int
    , name : String
    , pieceIds : List String
    , hue : Float
    , locked : Bool
    }


type PieceDisplay
    = SinglePiece String
    | GroupedPiece String (List String)


type AppMode
    = ModeInit
    | ModeGenerate
    | ModePieces
    | ModeBlueprint
    | ModeGroups
    | ModeWaves
    | ModeExport



-- ── Model ───────────────────────────────────────────────────────────────────


type LoadState
    = Idle
    | Loading
    | Loaded LoadResponse
    | LoadError String


type GenerateState
    = NotGenerated
    | Compositing
    | Generated


type alias UndoSnapshot =
    { pieces : List Piece
    , waves : List Wave
    , nextWaveId : Int
    , groups : List Group
    , nextGroupId : Int
    , gridHue : Float
    , outlineHue : Float
    }


type alias Model =
    { selectedFileName : String
    , pdfFiles : List { name : String, path : String }
    , loadState : LoadState
    , targetCount : Int
    , minBorder : Int
    , seed : Int
    , generateState : GenerateState
    , pieces : List Piece
    , pieceGeneration : Int
    , bricksById : Dict String Brick
    , appMode : AppMode
    , showOutlines : Bool
    , showGrid : Bool
    , showNumbers : Bool
    , showLights : Bool
    , showGroupOverlay : Bool
    , showWaveOverlay : Bool
    , waves : List Wave
    , nextWaveId : Int
    , groups : List Group
    , nextGroupId : Int
    , selectedGroupId : Maybe Int
    , dragOverGroupId : Maybe (Maybe Int)
    , hoveredPieceId : Maybe String
    , hoveredBrickId : Maybe String
    , selectedPieceId : Maybe String
    , selectedWaveId : Maybe Int
    , editMode : Bool
    , editBrickIds : List String
    , editOriginalBrickIds : List String
    , editOriginalPieces : List Piece
    , editOriginalWaves : List Wave
    , editOriginalGroups : List Group
    , recomputing : Bool
    , exporting : Bool
    , exportCanvasHeight : String
    , exportLocation : String
    , exportHouseName : String
    , exportPosition : String
    , exportSpacing : String
    , draggingPieceId : Maybe String
    , dragOverWaveId : Maybe (Maybe Int)
    , dragInsertBeforeId : Maybe String
    , lasso : Maybe { x0 : Float, y0 : Float, x1 : Float, y1 : Float }
    , colorPicking : Maybe { target : ColorPickTarget, panelX : Float, panelY : Float, hueOnly : Bool }
    , gridHue : Float
    , outlineHue : Float
    , svgScale : Float
    , availableH : Float
    , houseUnitsHigh : Float
    , zoomLevel : Float
    , zoomGridActive : Bool
    , sessionKey : String
    , nextSessionId : Int
    , appVersion : String
    , isTauri : Bool
    , undoHistory : List UndoSnapshot
    , redoHistory : List UndoSnapshot
    }


init : { version : String, isTauri : Bool } -> ( Model, Cmd Msg )
init flags =
    ( { selectedFileName = ""
      , pdfFiles = []
      , loadState = Idle
      , targetCount = 60
      , minBorder = 10
      , seed = 42
      , generateState = NotGenerated
      , pieces = []
      , pieceGeneration = 0
      , bricksById = Dict.empty
      , appMode = ModeInit
      , showOutlines = True
      , showGrid = False
      , showNumbers = True
      , showLights = False
      , showGroupOverlay = True
      , showWaveOverlay = True

      , waves = []
      , nextWaveId = 1
      , groups = []
      , nextGroupId = 1
      , selectedGroupId = Nothing
      , dragOverGroupId = Nothing
      , hoveredPieceId = Nothing
      , hoveredBrickId = Nothing
      , selectedPieceId = Nothing
      , selectedWaveId = Nothing
      , editMode = False
      , editBrickIds = []
      , editOriginalBrickIds = []
      , editOriginalPieces = []
      , editOriginalWaves = []
      , editOriginalGroups = []
      , recomputing = False
      , exporting = False
      , exportCanvasHeight = "900"
      , exportLocation = "Rome"
      , exportHouseName = "NewHouse"
      , exportPosition = "0"
      , exportSpacing = "12.0"
      , draggingPieceId = Nothing
      , dragOverWaveId = Nothing
      , dragInsertBeforeId = Nothing
      , lasso = Nothing
      , colorPicking = Nothing
      , gridHue = 35.0
      , outlineHue = 210.0
      , svgScale = 1.0
      , availableH = 900.0
      , houseUnitsHigh = 15.5
      , zoomLevel = 1.0
      , zoomGridActive = False
      , sessionKey = ""
      , nextSessionId = 1
      , appVersion = flags.version
      , isTauri = flags.isTauri
      , undoHistory = []
      , redoHistory = []
      }
    , Cmd.batch
        [ fetchPdfList flags.isTauri
        , Task.perform GotViewport Browser.Dom.getViewport
        ]
    )




-- ── Msg ─────────────────────────────────────────────────────────────────────


type Msg
    = GotFileList (Result Http.Error (List { name : String, path : String }))
    | PickFile
    | FileSelected File
    | FileUploaded (Result Http.Error String)
    | LoadFile String
    | Reset
    | GotLoadResponse (Result Http.Error LoadResponse)
    | SetTargetCount String
    | SetMinBorder String
    | SetSeed String
    | RequestGenerate
    | GotMergeResponse (Result Http.Error MergeResponse)
    | GotViewport Browser.Dom.Viewport
    | SetAppMode AppMode
    | ToggleOutlines Bool
    | ToggleGrid Bool
    | ToggleNumbers Bool
    | ToggleLights Bool
    | SetSpecialHue ColorPickTarget Float
    | ToggleGroupOverlay Bool
    | ToggleWaveOverlay Bool
    | AddWave
    | ToggleWaveVisibility Int
    | SetHoveredPiece (Maybe String)
    | SetHoveredBrick (Maybe String)
    | SelectPiece String
    | SelectWave (Maybe Int)
    | AssignPieceToWave String
    | RemovePieceFromWave Int String
    | MoveWave Int Int
    | RemoveWave Int
    | StartEdit
    | RemoveBrickFromEdit String
    | MergePieceIntoEdit String
    | SaveEdit
    | CancelEdit
    | GotPiecePolygons (Result Http.Error (List ( String, List Point )))
    | SetExportCanvasHeight String
    | SetExportLocation String
    | SetExportHouseName String
    | SetExportPosition String
    | SetExportSpacing String
    | RequestExport
    | GotExportResponse (Result Http.Error ())
    | LogBrickClick String
    | DragPieceStart String
    | DragPieceEnd
    | DragEnterWave (Maybe Int)
    | DragEnterPiece String
    | DropOnWave (Maybe Int)
    | ToggleWaveLock Int
    | ToggleGroupLock Int
    | AddGroup
    | SelectGroup (Maybe Int)
    | RemoveGroup Int
    | MoveGroup Int Int
    | AssignPieceToGroup String
    | DragEnterGroup (Maybe Int)
    | DropOnGroup (Maybe Int)
    | AssignGroupToWave Int Int
    | LassoStart Float Float
    | LassoMove Float Float
    | LassoEnd
    | SetZoomLevel Float
    | SetZoomGridActive Bool
    | SetHouseUnitsHigh String
    | StartColorPick ColorPickTarget Float Float
    | ColorPickMove Float Float
    | EndColorPick
    | ScrollTrayBy Float
    | Undo
    | Redo
    | TauriResponse D.Value
    | NoOp



-- ── Ports ───────────────────────────────────────────────────────────────────


port logBrick : E.Value -> Cmd msg


port setTitle : String -> Cmd msg


port tauriInvoke : { command : String, args : E.Value, requestId : String } -> Cmd msg


port tauriResponse : (D.Value -> msg) -> Sub msg



scrollToBottom : Cmd Msg
scrollToBottom =
    Task.attempt (\_ -> NoOp)
        (Process.sleep 0
            |> Task.andThen (\_ -> Browser.Dom.setViewportOf "house-scroll" 0 999999)
        )


scrollTrayToEnd : Cmd Msg
scrollTrayToEnd =
    Task.attempt (\_ -> NoOp)
        (Process.sleep 0
            |> Task.andThen (\_ -> Browser.Dom.setViewportOf "wave-tray-scroll" 999999 0)
        )


-- ── Undo/Redo Helpers ────────────────────────────────────────────────────────


takeSnapshot : Model -> UndoSnapshot
takeSnapshot model =
    { pieces = model.pieces
    , waves = model.waves
    , nextWaveId = model.nextWaveId
    , groups = model.groups
    , nextGroupId = model.nextGroupId
    , gridHue = model.gridHue
    , outlineHue = model.outlineHue
    }


applySnapshot : UndoSnapshot -> Model -> Model
applySnapshot snap model =
    { model
        | pieces = snap.pieces
        , waves = snap.waves
        , nextWaveId = snap.nextWaveId
        , groups = snap.groups
        , nextGroupId = snap.nextGroupId
        , gridHue = snap.gridHue
        , outlineHue = snap.outlineHue
    }


{-| Wrap an update result to push the old model snapshot onto undo history
and clear the redo stack. -}
withUndo : Model -> ( Model, Cmd Msg ) -> ( Model, Cmd Msg )
withUndo oldModel ( newModel, cmd ) =
    ( { newModel
        | undoHistory = List.take 50 (takeSnapshot oldModel :: oldModel.undoHistory)
        , redoHistory = []
      }
    , cmd
    )


-- ── Update ──────────────────────────────────────────────────────────────────


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        GotFileList (Ok files) ->
            ( { model | pdfFiles = files }, Cmd.none )

        GotFileList (Err _) ->
            ( model, Cmd.none )

        PickFile ->
            if model.isTauri then
                -- In Tauri mode, open a native OS file-picker dialog via Rust.
                -- The response arrives as a TauriResponse with requestId "pick_file".
                ( model
                , tauriInvoke
                    { command = "pick_file"
                    , args = E.object []
                    , requestId = "pick_file"
                    }
                )

            else
                ( model, Select.file [ ".pdf", "application/pdf", ".ai", "application/illustrator" ] FileSelected )

        FileSelected file ->
            let
                baseModel =
                    { model
                        | selectedFileName = File.name file
                        , loadState = Loading
                        , generateState = NotGenerated
                        , pieces = []
                        , pieceGeneration = 0
                        , waves = []
                        , nextWaveId = 1
                        , selectedPieceId = Nothing
                        , selectedWaveId = Nothing
                        , editMode = False
                        , editBrickIds = []
                        , editOriginalBrickIds = []
                        , editOriginalPieces = []
                        , editOriginalWaves = []
                        , editOriginalGroups = []
                        , recomputing = False
                        , appMode = ModeInit
                    }
            in
            if model.isTauri then
                -- In Tauri mode, skip the HTTP upload; construct local path directly
                let
                    path =
                        "in/" ++ File.name file

                    key =
                        String.fromInt model.nextSessionId
                in
                ( { baseModel | sessionKey = key, nextSessionId = model.nextSessionId + 1 }
                , loadPdf True key path model.availableH
                )
            else
                ( baseModel, uploadFile file )

        FileUploaded (Ok path) ->
            let
                key =
                    String.fromInt model.nextSessionId
            in
            ( { model | sessionKey = key, nextSessionId = model.nextSessionId + 1 }
            , loadPdf model.isTauri key path model.availableH
            )

        FileUploaded (Err _) ->
            ( { model | loadState = Idle }, Cmd.none )

        Reset ->
            ( { model
                | selectedFileName = ""
                , loadState = Idle
                , generateState = NotGenerated
                , pieces = []
                , pieceGeneration = 0
                , waves = []
                , nextWaveId = 1
                , selectedPieceId = Nothing
                , selectedWaveId = Nothing
                , editMode = False
                , editBrickIds = []
                , editOriginalBrickIds = []
                , editOriginalPieces = []
                , editOriginalWaves = []
                , editOriginalGroups = []
                , recomputing = False
                , appMode = ModeInit
                , sessionKey = ""
              }
            , fetchPdfList model.isTauri
            )

        LoadFile path ->
            let
                -- Extract house name from path: "in/_NY2.ai" -> "NY2"
                baseName =
                    path
                        |> String.split "/"
                        |> List.reverse
                        |> List.head
                        |> Maybe.withDefault path
                        |> String.replace ".ai" ""
                        |> String.replace ".pdf" ""

                houseName =
                    if String.startsWith "_" baseName then
                        String.dropLeft 1 baseName

                    else
                        baseName

                key =
                    String.fromInt model.nextSessionId
            in
            ( { model
                | selectedFileName = path
                , loadState = Loading
                , generateState = NotGenerated
                , pieces = []
                , pieceGeneration = 0
                , waves = []
                , nextWaveId = 1
                , selectedPieceId = Nothing
                , selectedWaveId = Nothing
                , editMode = False
                , editBrickIds = []
                , editOriginalBrickIds = []
                , editOriginalPieces = []
                , editOriginalWaves = []
                , editOriginalGroups = []
                , recomputing = False
                , appMode = ModeInit
                , exportHouseName = houseName
                , sessionKey = key
                , nextSessionId = model.nextSessionId + 1
              }
            , loadPdf model.isTauri key path model.availableH
            )

        GotLoadResponse (Ok response) ->
            ( { model
                | loadState = Loaded response
                , sessionKey = response.key
                , bricksById =
                    response.bricks
                        |> List.map (\b -> ( b.id, b ))
                        |> Dict.fromList
                , appMode = ModeGenerate
                , houseUnitsHigh = response.houseUnitsHigh
                , generateState = NotGenerated
                , pieces = []
                , pieceGeneration = 0
                , waves = []
                , nextWaveId = 1
                , groups = []
                , nextGroupId = 1
                , selectedPieceId = Nothing
                , selectedWaveId = Nothing
                , selectedGroupId = Nothing
              }
            , setTitle (model.exportHouseName ++ " — House Puzzle Editor")
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
                        , waves = []
                        , nextWaveId = 1
                        , selectedPieceId = Nothing
                        , selectedWaveId = Nothing
                        , editMode = False
                        , editBrickIds = []
                        , editOriginalBrickIds = []
                        , editOriginalPieces = []
                        , editOriginalWaves = []
                        , editOriginalGroups = []
                        , recomputing = False
                      }
                    , mergeBricks model.isTauri model.sessionKey model.targetCount model.minBorder model.seed
                    )

                _ ->
                    ( model, Cmd.none )

        GotMergeResponse (Ok response) ->
            ( { model
                | pieces = List.map (withPieceUrls model.sessionKey) response.pieces
                , generateState = Generated
                , appMode = ModePieces
                , pieceGeneration = model.pieceGeneration + 1
                , recomputing = False
              }
            , Task.perform GotViewport Browser.Dom.getViewport
            )

        GotMergeResponse (Err _) ->
            ( { model | generateState = NotGenerated, recomputing = False }, Cmd.none )

        SetAppMode mode ->
            let
                baseModel =
                    { model | appMode = mode, editMode = False, editBrickIds = [], editOriginalBrickIds = [], editOriginalPieces = [], editOriginalWaves = [], editOriginalGroups = [] }

                recomputeViewport =
                    Task.perform GotViewport Browser.Dom.getViewport
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
                                , hue = defaultHue (model.nextWaveId - 1)
                                , opacity = 0.3
                                }
                        in
                        ( { baseModel | waves = [ newWave ], nextWaveId = model.nextWaveId + 1, selectedWaveId = Just newWave.id }, recomputeViewport )

                    first :: _ ->
                        ( { baseModel | selectedWaveId = if baseModel.selectedWaveId == Nothing then Just first.id else baseModel.selectedWaveId }
                        , recomputeViewport
                        )

            else if mode == ModeGroups then
                case model.groups of
                    [] ->
                        let
                            newGroup =
                                { id = model.nextGroupId
                                , name = "Group " ++ String.fromInt model.nextGroupId
                                , pieceIds = []
                                , hue = defaultHue (model.nextGroupId - 1)
                                , locked = False
                                }
                        in
                        ( { baseModel | groups = [ newGroup ], nextGroupId = model.nextGroupId + 1, selectedGroupId = Just newGroup.id }, recomputeViewport )

                    first :: _ ->
                        ( { baseModel | selectedGroupId = if baseModel.selectedGroupId == Nothing then Just first.id else baseModel.selectedGroupId }
                        , recomputeViewport
                        )

            else
                ( baseModel, recomputeViewport )

        ToggleOutlines checked ->
            ( { model | showOutlines = checked }, Cmd.none )

        ToggleGrid checked ->
            ( { model | showGrid = checked }, Cmd.none )

        ToggleNumbers checked ->
            ( { model | showNumbers = checked }, Cmd.none )

        ToggleLights checked ->
            ( { model | showLights = checked }, Cmd.none )

        ToggleGroupOverlay checked ->
            ( { model | showGroupOverlay = checked }, Cmd.none )

        ToggleWaveOverlay checked ->
            ( { model | showWaveOverlay = checked }, Cmd.none )

        AddWave ->
            let
                newWave =
                    { id = model.nextWaveId
                    , name = "Wave " ++ String.fromInt model.nextWaveId
                    , visible = True
                    , locked = False
                    , pieceIds = []
                    , hue = defaultHue (model.nextWaveId - 1)
                    , opacity = 0.3
                    }

                lockedWaves =
                    List.map
                        (\w ->
                            if Just w.id == model.selectedWaveId then
                                { w | locked = True }
                            else
                                w
                        )
                        model.waves
            in
            withUndo model
                ( { model
                    | waves = [ newWave ] ++ lockedWaves
                    , nextWaveId = model.nextWaveId + 1
                    , selectedWaveId = Just newWave.id
                  }
                , Cmd.none
                )

        ToggleWaveVisibility waveId ->
            withUndo model
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

        SetHoveredBrick mid ->
            ( { model | hoveredBrickId = mid }, Cmd.none )

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
                        didAdd =
                            not targetLocked && not alreadyIn && not sourceLocked
                    in
                    withUndo model
                        ( { model | waves = updatedWaves }, if didAdd then scrollTrayToEnd else Cmd.none )

        RemovePieceFromWave wid pid ->
            let
                waveLocked =
                    model.waves |> List.any (\w -> w.id == wid && w.locked)
            in
            if waveLocked then
                ( model, Cmd.none )

            else
            withUndo model
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
            withUndo model ( { model | waves = renumbered }, Cmd.none )

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
            withUndo model ( { model | waves = renumbered, selectedWaveId = newSelectedWaveId }, Cmd.none )

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
                                , editOriginalPieces = model.pieces
                                , editOriginalWaves = model.waves
                                , editOriginalGroups = model.groups
                              }
                            , Cmd.none
                            )

        RemoveBrickFromEdit bid ->
            case model.selectedPieceId of
                Nothing ->
                    ( model, Cmd.none )

                Just editedPieceId ->
                    if List.length model.editBrickIds <= 1 then
                        -- Don't remove the last brick
                        ( model, Cmd.none )

                    else
                        let
                            newEditBrickIds =
                                List.filter (\b -> b /= bid) model.editBrickIds

                            -- Compute new ID for the displaced single-brick piece
                            maxIdNum =
                                List.foldl
                                    (\p acc ->
                                        case String.toInt (String.dropLeft 1 p.id) of
                                            Just n -> Basics.max n acc
                                            Nothing -> acc
                                    )
                                    0
                                    model.pieces

                            newPieceId =
                                "p" ++ String.fromInt (maxIdNum + 1)

                            -- Build a new single-brick piece for the displaced brick
                            newSinglePiece =
                                case Dict.get bid model.bricksById of
                                    Just brick ->
                                        { id = newPieceId
                                        , x = brick.x
                                        , y = brick.y
                                        , width = brick.width
                                        , height = brick.height
                                        , brickIds = [ bid ]
                                        , bricks = [ BrickRef bid brick.x brick.y brick.width brick.height ]
                                        , polygon = []
                                        , imgUrl = "/api/s/" ++ model.sessionKey ++ "/piece/" ++ newPieceId ++ ".png"
                                        , outlineUrl = "/api/s/" ++ model.sessionKey ++ "/piece_outline/" ++ newPieceId ++ ".png"
                                        }

                                    Nothing ->
                                        { id = newPieceId
                                        , x = 0
                                        , y = 0
                                        , width = 0
                                        , height = 0
                                        , brickIds = [ bid ]
                                        , bricks = []
                                        , polygon = []
                                        , imgUrl = "/api/s/" ++ model.sessionKey ++ "/piece/" ++ newPieceId ++ ".png"
                                        , outlineUrl = "/api/s/" ++ model.sessionKey ++ "/piece_outline/" ++ newPieceId ++ ".png"
                                        }

                            -- Update the edited piece in model.pieces
                            updatedPieces =
                                List.map
                                    (\p ->
                                        if p.id == editedPieceId then
                                            recalcPieceBbox model.sessionKey model.bricksById { p | brickIds = newEditBrickIds }
                                        else
                                            p
                                    )
                                    model.pieces
                                    ++ [ newSinglePiece ]
                        in
                        ( { model
                            | editBrickIds = newEditBrickIds
                            , pieces = updatedPieces
                          }
                        , Cmd.none
                        )

        MergePieceIntoEdit pid ->
            case model.selectedPieceId of
                Nothing ->
                    ( model, Cmd.none )

                Just editedPieceId ->
                    if pid == editedPieceId then
                        ( model, Cmd.none )

                    else
                        case List.filter (\p -> p.id == pid) model.pieces |> List.head of
                            Nothing ->
                                ( model, Cmd.none )

                            Just mergedPiece ->
                                let
                                    newEditBrickIds =
                                        model.editBrickIds ++ mergedPiece.brickIds

                                    -- Update edited piece and remove merged piece
                                    updatedPieces =
                                        List.map
                                            (\p ->
                                                if p.id == editedPieceId then
                                                    recalcPieceBbox model.sessionKey model.bricksById { p | brickIds = newEditBrickIds }
                                                else
                                                    p
                                            )
                                            model.pieces
                                            |> List.filter (\p -> p.id /= pid)

                                    -- Remove merged piece from waves
                                    updatedWaves =
                                        List.map
                                            (\w -> { w | pieceIds = List.filter (\wid -> wid /= pid) w.pieceIds })
                                            model.waves

                                    -- Remove merged piece from groups
                                    updatedGroups =
                                        List.map
                                            (\g -> { g | pieceIds = List.filter (\gid -> gid /= pid) g.pieceIds })
                                            model.groups
                                in
                                ( { model
                                    | editBrickIds = newEditBrickIds
                                    , pieces = updatedPieces
                                    , waves = updatedWaves
                                    , groups = updatedGroups
                                  }
                                , Cmd.none
                                )

        SaveEdit ->
            -- Pieces are already live-updated; just recompute polygons and clean up.
            let
                allPieces =
                    model.pieces
                        |> List.filter (\p -> not (List.isEmpty p.brickIds))

                -- Prune stale wave/group piece references
                validIds =
                    List.map .id allPieces

                updatedWaves =
                    List.map
                        (\w -> { w | pieceIds = List.filter (\wid -> List.member wid validIds) w.pieceIds })
                        model.waves

                updatedGroups =
                    List.map
                        (\g -> { g | pieceIds = List.filter (\gid -> List.member gid validIds) g.pieceIds })
                        model.groups
            in
            withUndo model
                ( { model
                    | pieces = allPieces
                    , waves = updatedWaves
                    , groups = updatedGroups
                    , editMode = False
                    , editBrickIds = []
                    , editOriginalBrickIds = []
                    , editOriginalPieces = []
                    , editOriginalWaves = []
                    , editOriginalGroups = []
                    , generateState = Generated
                    , recomputing = True
                  }
                , recomputePiecePolygons model.isTauri model.sessionKey allPieces
                )

        CancelEdit ->
            ( { model
                | editMode = False
                , editBrickIds = []
                , editOriginalBrickIds = []
                , editOriginalPieces = []
                , editOriginalWaves = []
                , editOriginalGroups = []
                , pieces = model.editOriginalPieces
                , waves = model.editOriginalWaves
                , groups = model.editOriginalGroups
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
            ( { model | pieces = updatedPieces, recomputing = False, pieceGeneration = model.pieceGeneration + 1 }, Cmd.none )

        GotPiecePolygons (Err _) ->
            ( { model | recomputing = False }, Cmd.none )

        SetExportCanvasHeight s ->
            ( { model | exportCanvasHeight = s }, Cmd.none )

        SetExportLocation s ->
            ( { model | exportLocation = s }, Cmd.none )

        SetExportHouseName s ->
            ( { model | exportHouseName = s }, Cmd.none )

        SetExportPosition s ->
            ( { model | exportPosition = s }, Cmd.none )

        SetExportSpacing s ->
            ( { model | exportSpacing = s }, Cmd.none )

        RequestExport ->
            let
                wavesJson =
                    E.list
                        (\( idx, wv ) ->
                            E.object
                                [ ( "wave", E.int (idx + 1) )
                                , ( "pieceIds", E.list E.string wv.pieceIds )
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

                exportHeight =
                    Maybe.withDefault 900 (String.toInt model.exportCanvasHeight)

                groupsJson =
                    E.list
                        (\g ->
                            E.object
                                [ ( "pieceIds", E.list E.string g.pieceIds )
                                ]
                        )
                        model.groups

                payload =
                    E.object
                        [ ( "waves", wavesJson )
                        , ( "outlines", outlinesJson )
                        , ( "groups", groupsJson )
                        , ( "export_canvas_height", E.int exportHeight )
                        , ( "placement"
                          , E.object
                                [ ( "location", E.string model.exportLocation )
                                , ( "position", E.int (Maybe.withDefault 0 (String.toInt model.exportPosition)) )
                                , ( "houseName", E.string model.exportHouseName )
                                , ( "spacing", E.float (Maybe.withDefault 12.0 (String.toFloat model.exportSpacing)) )
                                ]
                          )
                        ]
            in
            if model.isTauri then
                ( { model | exporting = True }
                , tauriInvoke
                    { command = "export_data"
                    , args =
                        E.object
                            [ ( "key", E.string model.sessionKey )
                            , ( "waves", wavesJson )
                            , ( "groups", groupsJson )
                            , ( "export_canvas_height", E.int exportHeight )
                            , ( "placement"
                              , E.object
                                    [ ( "location", E.string model.exportLocation )
                                    , ( "position", E.int (Maybe.withDefault 0 (String.toInt model.exportPosition)) )
                                    , ( "houseName", E.string model.exportHouseName )
                                    , ( "spacing", E.float (Maybe.withDefault 12.0 (String.toFloat model.exportSpacing)) )
                                    ]
                              )
                            ]
                    , requestId = "export"
                    }
                )

            else
                ( { model | exporting = True }
                , Http.riskyRequest
                    { method = "POST"
                    , headers = []
                    , url = "/api/s/" ++ model.sessionKey ++ "/export"
                    , body = Http.jsonBody payload
                    , expect = Http.expectWhatever GotExportResponse
                    , timeout = Just (10 * 60 * 1000)
                    , tracker = Nothing
                    }
                )

        GotExportResponse _ ->
            ( { model | exporting = False }, Cmd.none )

        LogBrickClick brickId ->
            let
                maybeBrick =
                    case model.loadState of
                        Loaded r -> List.filter (\b -> b.id == brickId) r.bricks |> List.head
                        _ -> Nothing
            in
            ( model
            , logBrick
                (E.object
                    [ ( "brickId", E.string brickId )
                    , ( "layerName", maybeBrick |> Maybe.map (.layerName >> E.string) |> Maybe.withDefault E.null )
                    , ( "pieceId"
                      , model.pieces
                            |> List.filter (\p -> List.any (\br -> br.id == brickId) p.bricks)
                            |> List.head
                            |> Maybe.map (.id >> E.string)
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
                        -- If the dragged piece is part of a group, move the whole group
                        maybeGroup =
                            model.groups |> List.filter (\g -> List.member pid g.pieceIds) |> List.head

                        pidsToMove =
                            case maybeGroup of
                                Just g -> g.pieceIds
                                Nothing -> [ pid ]

                        insertBefore =
                            model.dragInsertBeforeId

                        insertInto wvPids =
                            let
                                filtered =
                                    List.filter (\p -> not (List.member p pidsToMove)) wvPids
                            in
                            case insertBefore of
                                Just beforeId ->
                                    if List.member beforeId pidsToMove then
                                        filtered ++ pidsToMove
                                    else
                                        List.concatMap
                                            (\p ->
                                                if p == beforeId then
                                                    pidsToMove ++ [ p ]
                                                else
                                                    [ p ]
                                            )
                                            filtered

                                Nothing ->
                                    filtered ++ pidsToMove

                        targetIsLocked =
                            case targetWaveId of
                                Just wid ->
                                    model.waves |> List.any (\wv -> wv.id == wid && wv.locked)

                                Nothing ->
                                    False

                        sourceIsLocked =
                            model.waves |> List.any (\wv -> List.any (\p -> List.member p pidsToMove) wv.pieceIds && wv.locked)

                        newWaves =
                            if targetIsLocked || sourceIsLocked then
                                model.waves

                            else
                                model.waves
                                    |> List.map (\wv -> { wv | pieceIds = List.filter (\p -> not (List.member p pidsToMove)) wv.pieceIds })
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
                    withUndo model
                        ( { model | waves = newWaves, draggingPieceId = Nothing, dragOverWaveId = Nothing, dragInsertBeforeId = Nothing }, Cmd.none )

        ToggleWaveLock wid ->
            withUndo model
                ( { model | waves = List.map (\w -> if w.id == wid then { w | locked = not w.locked } else w) model.waves }
                , Cmd.none
                )

        ToggleGroupLock gid ->
            withUndo model
                ( { model | groups = List.map (\g -> if g.id == gid then { g | locked = not g.locked } else g) model.groups }
                , Cmd.none
                )

        AddGroup ->
            let
                newGroup =
                    { id = model.nextGroupId
                    , name = "Group " ++ String.fromInt model.nextGroupId
                    , pieceIds = []
                    , hue = defaultHue (model.nextGroupId - 1)
                    , locked = False
                    }
            in
            withUndo model
                ( { model
                    | groups = model.groups ++ [ newGroup ]
                    , nextGroupId = model.nextGroupId + 1
                    , selectedGroupId = Just newGroup.id
                  }
                , Cmd.none
                )

        SelectGroup mgid ->
            ( { model | selectedGroupId = mgid }, Cmd.none )

        RemoveGroup gid ->
            let
                newGroups =
                    List.filter (\g -> g.id /= gid) model.groups

                newSelectedGroupId =
                    if model.selectedGroupId == Just gid then
                        List.head newGroups |> Maybe.map .id
                    else
                        model.selectedGroupId
            in
            withUndo model ( { model | groups = newGroups, selectedGroupId = newSelectedGroupId }, Cmd.none )

        MoveGroup gid dir ->
            let
                moveItem lst =
                    let
                        indexed = List.indexedMap Tuple.pair lst
                        idx = indexed |> List.filter (\( _, g ) -> g.id == gid) |> List.head |> Maybe.map Tuple.first |> Maybe.withDefault 0
                        newIdx = Basics.max 0 (Basics.min (List.length lst - 1) (idx + dir))
                        item = lst |> List.drop idx |> List.head
                        without = List.take idx lst ++ List.drop (idx + 1) lst
                    in
                    case item of
                        Just g -> List.take newIdx without ++ [ g ] ++ List.drop newIdx without
                        Nothing -> lst
            in
            withUndo model ( { model | groups = moveItem model.groups }, Cmd.none )

        AssignPieceToGroup pid ->
            case model.selectedGroupId of
                Nothing ->
                    ( model, Cmd.none )

                Just gid ->
                    let
                        alreadyIn =
                            model.groups |> List.any (\g -> g.id == gid && List.member pid g.pieceIds)

                        updatedGroups =
                            List.map
                                (\g ->
                                    if g.id == gid then
                                        if alreadyIn then
                                            { g | pieceIds = List.filter (\p -> p /= pid) g.pieceIds }
                                        else
                                            { g | pieceIds = g.pieceIds ++ [ pid ] }
                                    else if not alreadyIn then
                                        { g | pieceIds = List.filter (\p -> p /= pid) g.pieceIds }
                                    else
                                        g
                                )
                                model.groups
                    in
                    withUndo model ( { model | groups = updatedGroups }, Cmd.none )

        DragEnterGroup mgid ->
            ( { model | dragOverGroupId = Just mgid }, Cmd.none )

        DropOnGroup mgid ->
            case model.draggingPieceId of
                Nothing ->
                    ( { model | dragOverGroupId = Nothing }, Cmd.none )

                Just pid ->
                    let
                        updatedGroups =
                            case mgid of
                                Nothing ->
                                    List.map (\g -> { g | pieceIds = List.filter ((/=) pid) g.pieceIds }) model.groups

                                Just gid ->
                                    List.map
                                        (\g ->
                                            if g.id == gid then
                                                if List.member pid g.pieceIds then g else { g | pieceIds = g.pieceIds ++ [ pid ] }
                                            else
                                                { g | pieceIds = List.filter ((/=) pid) g.pieceIds }
                                        )
                                        model.groups
                    in
                    withUndo model ( { model | groups = updatedGroups, draggingPieceId = Nothing, dragOverGroupId = Nothing }, Cmd.none )

        AssignGroupToWave gid wid ->
            case model.groups |> List.filter (\g -> g.id == gid) |> List.head of
                Nothing ->
                    ( model, Cmd.none )

                Just group ->
                    let
                        pids =
                            group.pieceIds

                        targetLocked =
                            model.waves |> List.any (\w -> w.id == wid && w.locked)

                        alreadyAll =
                            not (List.isEmpty pids)
                                && List.all (\pid -> model.waves |> List.any (\w -> w.id == wid && List.member pid w.pieceIds)) pids

                        updatedWaves =
                            if targetLocked then
                                model.waves

                            else if alreadyAll then
                                List.map
                                    (\w ->
                                        if w.id == wid then
                                            { w | pieceIds = List.filter (\p -> not (List.member p pids)) w.pieceIds }
                                        else
                                            w
                                    )
                                    model.waves

                            else
                                model.waves
                                    |> List.map (\w -> { w | pieceIds = List.filter (\p -> not (List.member p pids)) w.pieceIds })
                                    |> List.map (\w -> if w.id == wid then { w | pieceIds = w.pieceIds ++ pids } else w)
                    in
                    withUndo model
                        ( { model | waves = updatedWaves }
                        , if targetLocked then Cmd.none else scrollTrayToEnd
                        )

        GotViewport viewport ->
            let
                vh =
                    viewport.viewport.height

                -- Wave tray CSS: height = (100vh - 48) * 0.12  (only shown in waves mode)
                -- The 48px offset in that rule has unclear origin.
                -- Subtract canvas-area padding-bottom (16px).
                waveTrayOffset = 48
                waveTrayHeight = (vh - waveTrayOffset) * 0.12
                bottomPadding  = 16   -- .canvas-area padding-bottom

                availableH =
                    if model.appMode == ModeWaves then
                        vh - waveTrayHeight - bottomPadding
                    else
                        vh - bottomPadding
            in
            case model.loadState of
                Loaded response ->
                    let
                        svgH =
                            response.canvas.height + 20

                        scale =
                            availableH * model.houseUnitsHigh / (svgH * 15.5)
                    in
                    ( { model | svgScale = scale, availableH = availableH }
                    , scrollToBottom
                    )

                _ ->
                    ( { model | availableH = availableH }, Cmd.none )

        LassoStart x y ->
            if model.selectedWaveId /= Nothing then
                ( { model | lasso = Just { x0 = x, y0 = y, x1 = x, y1 = y } }, Cmd.none )

            else
                ( model, Cmd.none )

        LassoMove x y ->
            case model.lasso of
                Nothing ->
                    ( model, Cmd.none )

                Just ls ->
                    ( { model | lasso = Just { ls | x1 = x, y1 = y } }, Cmd.none )

        LassoEnd ->
            case model.lasso of
                Nothing ->
                    ( model, Cmd.none )

                Just ls ->
                    let
                        isDrag =
                            abs (ls.x1 - ls.x0) > 5 || abs (ls.y1 - ls.y0) > 5

                        cleared =
                            { model | lasso = Nothing }
                    in
                    if not isDrag then
                        ( cleared, Cmd.none )

                    else
                        case model.selectedWaveId of
                            Nothing ->
                                ( cleared, Cmd.none )

                            Just wid ->
                                let
                                    lx0 = Basics.min ls.x0 ls.x1
                                    lx1 = Basics.max ls.x0 ls.x1
                                    ly0 = Basics.min ls.y0 ls.y1
                                    ly1 = Basics.max ls.y0 ls.y1

                                    selectedIds =
                                        model.pieces
                                            |> List.filter (\p ->
                                                p.x < lx1
                                                    && (p.x + p.width) > lx0
                                                    && p.y < ly1
                                                    && (p.y + p.height) > ly0
                                            )
                                            |> List.map .id

                                    updatedWaves =
                                        List.foldl
                                            (\pid waves ->
                                                let
                                                    alreadyIn =
                                                        waves
                                                            |> List.filter (\w -> w.id == wid)
                                                            |> List.head
                                                            |> Maybe.map (\w -> List.member pid w.pieceIds)
                                                            |> Maybe.withDefault False

                                                    srcLocked =
                                                        waves |> List.any (\w -> w.locked && List.member pid w.pieceIds)

                                                    tgtLocked =
                                                        waves |> List.filter (\w -> w.id == wid) |> List.head |> Maybe.map .locked |> Maybe.withDefault False
                                                in
                                                if tgtLocked || (not alreadyIn && srcLocked) then
                                                    waves

                                                else if alreadyIn then
                                                    waves

                                                else
                                                    List.map
                                                        (\w ->
                                                            if w.id == wid then
                                                                { w | pieceIds = w.pieceIds ++ [ pid ] }

                                                            else
                                                                { w | pieceIds = List.filter (\p -> p /= pid) w.pieceIds }
                                                        )
                                                        waves
                                            )
                                            model.waves
                                            selectedIds
                                in
                                withUndo model ( { cleared | waves = updatedWaves }, Cmd.none )

        SetZoomLevel z ->
            ( { model | zoomLevel = z }, Cmd.none )

        SetZoomGridActive b ->
            ( { model | zoomGridActive = b }, Cmd.none )

        SetHouseUnitsHigh s ->
            case String.toFloat s of
                Just h ->
                    if h > 0 then
                        ( { model | houseUnitsHigh = h }
                        , Task.perform GotViewport Browser.Dom.getViewport
                        )

                    else
                        ( model, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )

        StartColorPick target px py ->
            let
                hueOnly =
                    target == GridColorTarget || target == OutlineColorTarget

                ( currentHue, currentOpacity ) =
                    case target of
                        WaveColorTarget waveId ->
                            model.waves
                                |> List.filter (\w -> w.id == waveId)
                                |> List.head
                                |> Maybe.map (\w -> ( w.hue, w.opacity ))
                                |> Maybe.withDefault ( 0, 0.3 )

                        GroupColorTarget groupId ->
                            model.groups
                                |> List.filter (\g -> g.id == groupId)
                                |> List.head
                                |> Maybe.map (\g -> ( g.hue, 1.0 ))
                                |> Maybe.withDefault ( 0, 1.0 )

                        GridColorTarget ->
                            ( model.gridHue, 1.0 )

                        OutlineColorTarget ->
                            ( model.outlineHue, 1.0 )

                innerH =
                    if hueOnly then 20 else 96

                panelX =
                    px - 10 - (currentHue / 360) * 240

                panelY =
                    py - 10 - (1 - currentOpacity) * toFloat innerH
            in
            ( { model | colorPicking = Just { target = target, panelX = panelX, panelY = panelY, hueOnly = hueOnly } }, Cmd.none )

        ColorPickMove mx my ->
            case model.colorPicking of
                Nothing ->
                    ( model, Cmd.none )

                Just cp ->
                    let
                        -- B/W zone: first 40px (2×20, no gaps)
                        localX = mx - cp.panelX - 10
                        newHue =
                            if localX < 20 then -2  -- black swatch
                            else if localX < 40 then -1  -- white swatch
                            else clamp 0 360 ((localX - 40) / 240 * 360)

                        newOpacity =
                            if cp.hueOnly then
                                1.0

                            else
                                clamp 0.05 1.0 (1.0 - (my - cp.panelY - 10) / 96)

                    in
                    case cp.target of
                        WaveColorTarget waveId ->
                            ( { model
                                | waves =
                                    List.map
                                        (\w ->
                                            if w.id == waveId then
                                                { w | hue = newHue, opacity = newOpacity }

                                            else
                                                w
                                        )
                                        model.waves
                              }
                            , Cmd.none
                            )

                        GroupColorTarget groupId ->
                            ( { model
                                | groups =
                                    List.map
                                        (\g ->
                                            if g.id == groupId then
                                                { g | hue = newHue }

                                            else
                                                g
                                        )
                                        model.groups
                              }
                            , Cmd.none
                            )

                        GridColorTarget ->
                            ( { model | gridHue = newHue }, Cmd.none )

                        OutlineColorTarget ->
                            ( { model | outlineHue = newHue }, Cmd.none )

        SetSpecialHue target hue ->
            let
                updated =
                    case target of
                        GridColorTarget ->
                            { model | gridHue = hue, colorPicking = Nothing }

                        OutlineColorTarget ->
                            { model | outlineHue = hue, colorPicking = Nothing }

                        WaveColorTarget wid ->
                            { model
                                | waves = List.map (\w -> if w.id == wid then { w | hue = hue } else w) model.waves
                                , colorPicking = Nothing
                            }

                        GroupColorTarget gid ->
                            { model
                                | groups = List.map (\g -> if g.id == gid then { g | hue = hue } else g) model.groups
                                , colorPicking = Nothing
                            }
            in
            withUndo model ( updated, Cmd.none )

        EndColorPick ->
            ( { model | colorPicking = Nothing }, Cmd.none )

        ScrollTrayBy delta ->
            ( model
            , Task.attempt (\_ -> NoOp)
                (Browser.Dom.getViewportOf "wave-tray-scroll"
                    |> Task.andThen (\vp -> Browser.Dom.setViewportOf "wave-tray-scroll" (vp.viewport.x + delta) 0)
                )
            )

        Undo ->
            case model.undoHistory of
                [] ->
                    ( model, Cmd.none )

                top :: rest ->
                    let
                        currentSnap =
                            takeSnapshot model

                        restored =
                            applySnapshot top model
                    in
                    ( { restored
                        | undoHistory = rest
                        , redoHistory = List.take 50 (currentSnap :: model.redoHistory)
                      }
                    , Cmd.none
                    )

        Redo ->
            case model.redoHistory of
                [] ->
                    ( model, Cmd.none )

                top :: rest ->
                    let
                        currentSnap =
                            takeSnapshot model

                        restored =
                            applySnapshot top model
                    in
                    ( { restored
                        | undoHistory = List.take 50 (currentSnap :: model.undoHistory)
                        , redoHistory = rest
                      }
                    , Cmd.none
                    )

        TauriResponse val ->
            let
                requestId =
                    D.decodeValue (D.field "requestId" D.string) val
                        |> Result.withDefault ""

                ok =
                    D.decodeValue (D.field "ok" D.bool) val
                        |> Result.withDefault False

                dataVal =
                    D.decodeValue (D.field "data" D.value) val
                        |> Result.withDefault E.null

                errorStr =
                    D.decodeValue (D.field "error" D.string) val
                        |> Result.withDefault "Tauri invoke error"
            in
            if not ok then
                case requestId of
                    "load_pdf" ->
                        ( { model | loadState = LoadError errorStr }, Cmd.none )

                    "merge_pieces" ->
                        ( { model | generateState = NotGenerated, recomputing = False }, Cmd.none )

                    "merge_pieces_recompute" ->
                        ( { model | recomputing = False }, Cmd.none )

                    "export" ->
                        ( { model | exporting = False }, Cmd.none )

                    _ ->
                        ( model, Cmd.none )

            else
                case requestId of
                    "list_pdfs" ->
                        case D.decodeValue (D.field "files" (D.list decodePdfFile)) dataVal of
                            Ok files ->
                                ( { model | pdfFiles = files }, Cmd.none )

                            Err _ ->
                                ( model, Cmd.none )

                    "load_pdf" ->
                        case D.decodeValue decodeLoadResponse dataVal of
                            Ok response ->
                                update (GotLoadResponse (Ok response)) model

                            Err e ->
                                ( { model | loadState = LoadError (D.errorToString e) }, Cmd.none )

                    "merge_pieces" ->
                        case D.decodeValue decodeMergeResponse dataVal of
                            Ok response ->
                                update (GotMergeResponse (Ok response)) model

                            Err _ ->
                                ( { model | generateState = NotGenerated, recomputing = False }, Cmd.none )

                    "merge_pieces_recompute" ->
                        -- In Tauri mode, get full merge response and update polygon + images
                        case D.decodeValue decodeMergeResponse dataVal of
                            Ok response ->
                                let
                                    pieceMap =
                                        Dict.fromList (List.map (\p -> ( p.id, p )) response.pieces)

                                    updatedPieces =
                                        List.map
                                            (\p ->
                                                case Dict.get p.id pieceMap of
                                                    Just rp ->
                                                        { p
                                                            | polygon = rp.polygon
                                                            , imgUrl =
                                                                if String.isEmpty rp.imgUrl then
                                                                    p.imgUrl
                                                                else
                                                                    rp.imgUrl
                                                            , outlineUrl =
                                                                if String.isEmpty rp.outlineUrl then
                                                                    p.outlineUrl
                                                                else
                                                                    rp.outlineUrl
                                                        }

                                                    Nothing ->
                                                        p
                                            )
                                            model.pieces
                                in
                                ( { model
                                    | pieces = updatedPieces
                                    , recomputing = False
                                    , pieceGeneration = model.pieceGeneration + 1
                                  }
                                , Cmd.none
                                )

                            Err _ ->
                                ( { model | recomputing = False }, Cmd.none )

                    "export" ->
                        ( { model | exporting = False }, Cmd.none )

                    "pick_file" ->
                        -- Native dialog result: null when cancelled, path string when selected.
                        case D.decodeValue (D.nullable D.string) dataVal of
                            Ok (Just path) ->
                                let
                                    -- Extract the filename from the full OS path (handles / and \).
                                    fileName =
                                        path
                                            |> String.split "/"
                                            |> List.reverse
                                            |> List.head
                                            |> Maybe.withDefault path
                                            |> (\n ->
                                                    String.split "\\" n
                                                        |> List.reverse
                                                        |> List.head
                                                        |> Maybe.withDefault n
                                               )

                                    key =
                                        String.fromInt model.nextSessionId

                                    baseModel =
                                        { model
                                            | selectedFileName = fileName
                                            , loadState = Loading
                                            , generateState = NotGenerated
                                            , pieces = []
                                            , pieceGeneration = 0
                                            , waves = []
                                            , nextWaveId = 1
                                            , selectedPieceId = Nothing
                                            , selectedWaveId = Nothing
                                            , editMode = False
                                            , editBrickIds = []
                                            , editOriginalBrickIds = []
                                            , editOriginalPieces = []
                                            , editOriginalWaves = []
                                            , editOriginalGroups = []
                                            , recomputing = False
                                            , appMode = ModeInit
                                            , sessionKey = key
                                            , nextSessionId = model.nextSessionId + 1
                                        }
                                in
                                ( baseModel, loadPdf True key path model.availableH )

                            _ ->
                                -- User cancelled the dialog — stay in current state.
                                ( model, Cmd.none )

                    _ ->
                        ( model, Cmd.none )

        NoOp ->
            ( model, Cmd.none )



-- ── Helpers ─────────────────────────────────────────────────────────────────


cacheBust : String -> Int -> String
cacheBust url gen =
    if String.startsWith "data:" url then
        url
    else
        url ++ "?v=" ++ String.fromInt gen


withPieceUrls : String -> Piece -> Piece
withPieceUrls key p =
    { p
        | imgUrl =
            -- Preserve data URLs (set by Tauri) or already-set HTTP URLs
            if String.isEmpty p.imgUrl then
                "/api/s/" ++ key ++ "/piece/" ++ p.id ++ ".png"
            else
                p.imgUrl
        , outlineUrl =
            if String.isEmpty p.outlineUrl then
                "/api/s/" ++ key ++ "/piece_outline/" ++ p.id ++ ".png"
            else
                p.outlineUrl
    }


recalcPieceBbox : String -> Dict String Brick -> Piece -> Piece
recalcPieceBbox sessionKey bricksById piece =
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
                    { piece | x = x, y = y, width = x2 - x, height = y2 - y, bricks = newBrickRefs, polygon = [], imgUrl = "/api/s/" ++ sessionKey ++ "/piece/" ++ piece.id ++ ".png", outlineUrl = "/api/s/" ++ sessionKey ++ "/piece_outline/" ++ piece.id ++ ".png" }

                _ ->
                    piece


editHasChanges : Model -> Bool
editHasChanges model =
    List.sort model.editBrickIds /= List.sort model.editOriginalBrickIds



-- ── HTTP ────────────────────────────────────────────────────────────────────


fetchPdfList : Bool -> Cmd Msg
fetchPdfList isTauri =
    if isTauri then
        tauriInvoke
            { command = "list_pdfs"
            , args = E.object []
            , requestId = "list_pdfs"
            }

    else
        Http.get
            { url = "/api/list_pdfs"
            , expect = Http.expectJson GotFileList (D.field "files" (D.list decodePdfFile))
            }


decodePdfFile : D.Decoder { name : String, path : String }
decodePdfFile =
    D.map2 (\n p -> { name = n, path = p })
        (D.field "name" D.string)
        (D.field "path" D.string)


uploadFile : File -> Cmd Msg
uploadFile file =
    Http.post
        { url = "/api/upload_file"
        , body = Http.multipartBody [ Http.filePart "file" file ]
        , expect = Http.expectJson FileUploaded (D.field "path" D.string)
        }


loadPdf : Bool -> String -> String -> Float -> Cmd Msg
loadPdf isTauri key path canvasHeight =
    if isTauri then
        tauriInvoke
            { command = "load_pdf"
            , args =
                E.object
                    [ ( "path", E.string path )
                    , ( "canvas_height", E.int (round canvasHeight) )
                    ]
            , requestId = "load_pdf"
            }

    else
        Http.riskyRequest
            { method = "POST"
            , headers = []
            , url = "/api/s/" ++ key ++ "/load"
            , body =
                Http.jsonBody
                    (E.object
                        [ ( "path", E.string path )
                        , ( "canvas_height", E.int (round canvasHeight) )
                        ]
                    )
            , expect = Http.expectJson GotLoadResponse decodeLoadResponse
            , timeout = Just (5 * 60 * 1000)
            , tracker = Nothing
            }


mergeBricks : Bool -> String -> Int -> Int -> Int -> Cmd Msg
mergeBricks isTauri key targetCount minBorder seed =
    if isTauri then
        tauriInvoke
            { command = "merge_pieces"
            , args =
                E.object
                    [ ( "key", E.string key )
                    , ( "target_count", E.int targetCount )
                    , ( "seed", E.int seed )
                    , ( "min_border", E.int minBorder )
                    ]
            , requestId = "merge_pieces"
            }

    else
        Http.post
            { url = "/api/s/" ++ key ++ "/merge"
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



recomputePiecePolygons : Bool -> String -> List Piece -> Cmd Msg
recomputePiecePolygons isTauri key pieces =
    let
        piecesArg =
            E.list
                (\p ->
                    E.object
                        [ ( "id", E.string p.id )
                        , ( "brick_ids", E.list E.string p.brickIds )
                        ]
                )
                pieces
    in
    if isTauri then
        tauriInvoke
            { command = "merge_pieces"
            , args =
                E.object
                    [ ( "key", E.string key )
                    , ( "pieces", piecesArg )
                    ]
            , requestId = "merge_pieces_recompute"
            }

    else
        Http.post
            { url = "/api/s/" ++ key ++ "/merge"
            , body =
                Http.jsonBody
                    (E.object
                        [ ( "pieces", piecesArg )
                        ]
                    )
            , expect = Http.expectJson GotPiecePolygons decodePiecePolygonResponse
            }


decodePiecePolygonResponse : D.Decoder (List ( String, List Point ))
decodePiecePolygonResponse =
    D.field "pieces"
        (D.list
            (D.map2 Tuple.pair
                (D.field "id" D.string)
                (D.field "polygon" (D.list decodePoint))
            )
        )


-- ── Decoders ────────────────────────────────────────────────────────────────


decodeLoadResponse : D.Decoder LoadResponse
decodeLoadResponse =
    D.map8
        (\canvas bricks hasComposite hasBase renderDpi warnings outlinesUrl compositeUrl ->
            \blueprintBgUrl lightsUrl ->
                \houseUnitsHigh key ->
                    LoadResponse canvas bricks hasComposite hasBase renderDpi warnings outlinesUrl compositeUrl blueprintBgUrl lightsUrl houseUnitsHigh key
        )
        (D.field "canvas" decodeCanvas)
        (D.field "bricks" (D.list decodeBrick))
        (D.field "has_composite" D.bool)
        (D.field "has_base" D.bool)
        (D.field "render_dpi" D.float)
        (D.field "warnings" (D.list D.string))
        (D.field "outlines_url" D.string |> D.maybe |> D.map (Maybe.withDefault "/api/outlines.png"))
        (D.field "composite_url" D.string |> D.maybe |> D.map (Maybe.withDefault "/api/composite.png"))
        |> D.andThen (\f -> D.map f (D.field "blueprint_bg_url" D.string |> D.maybe))
        |> D.andThen (\f -> D.map f (D.field "lights_url" D.string |> D.maybe))
        |> D.andThen (\f -> D.map f (D.field "houseUnitsHigh" D.float |> D.maybe |> D.map (Maybe.withDefault 15.5)))
        |> D.andThen (\f -> D.map f (D.field "key" D.string))


decodeCanvas : D.Decoder Canvas
decodeCanvas =
    D.map2 Canvas
        (D.field "width" D.float)
        (D.field "height" D.float)


decodeBrick : D.Decoder Brick
decodeBrick =
    D.map8 (\id x y w h t n p -> Brick id x y w h t n p "")
        (D.field "id" D.string)
        (D.field "x" D.float)
        (D.field "y" D.float)
        (D.field "width" D.float)
        (D.field "height" D.float)
        (D.field "type" D.string)
        (D.field "neighbors" (D.list D.string))
        (D.field "polygon" (D.list decodePoint))
        |> D.andThen (\brick ->
            D.map (\ln -> { brick | layerName = ln })
                (D.oneOf [ D.field "layer_name" D.string, D.succeed "" ])
        )


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
    D.map8
        (\id_ x_ y_ w_ h_ brickIds_ bricks_ polygon_ ->
            { id = id_
            , x = x_
            , y = y_
            , width = w_
            , height = h_
            , brickIds = brickIds_
            , bricks = bricks_
            , polygon = polygon_
            , imgUrl = ""
            , outlineUrl = ""
            }
        )
        (D.field "id" D.string)
        (D.field "x" D.float)
        (D.field "y" D.float)
        (D.field "width" D.float)
        (D.field "height" D.float)
        (D.field "brick_ids" (D.list D.string))
        (D.field "bricks" (D.list decodeBrickRef))
        (D.field "polygon" (D.list decodePoint))
        |> D.andThen (\piece ->
            D.map2 (\img out -> { piece | imgUrl = img, outlineUrl = out })
                (D.oneOf [ D.field "img_url" D.string, D.succeed "" ])
                (D.oneOf [ D.field "outline_url" D.string, D.succeed "" ])
        )


decodeBrickRef : D.Decoder BrickRef
decodeBrickRef =
    D.map5 BrickRef
        (D.field "id" D.string)
        (D.field "x" D.float)
        (D.field "y" D.float)
        (D.field "width" D.float)
        (D.field "height" D.float)



-- ── Encoders ────────────────────────────────────────────────────────────────


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
        ([ div [ class "app-main" ]
             [ viewTitleBar model
             , viewBody model
             , viewColorPickerPanel model
             ]
         ]
            ++ viewBottomWaveTray model
        )


viewBottomWaveTray : Model -> List (Html Msg)
viewBottomWaveTray model =
    if model.appMode /= ModeWaves then
        []
    else
        case model.loadState of
            Loaded response ->
                [ viewWaveTray model response ]
            _ ->
                []


viewColorPickerPanel : Model -> Html Msg
viewColorPickerPanel model =
    case model.colorPicking of
        Nothing ->
            text ""

        Just cp ->
            div
                [ class "color-picker-panel"
                , style "left" (String.fromFloat cp.panelX ++ "px")
                , style "top" (String.fromFloat cp.panelY ++ "px")
                ]
                [ div [ class "color-picker-row" ]
                    [ div [ class "color-picker-bw" ]
                        [ div [ class "bw-swatch bw-black", title "Black" ] []
                        , div [ class "bw-swatch bw-white", title "White" ] []
                        ]
                    , div [ class (if cp.hueOnly then "color-picker-inner hue-only" else "color-picker-inner") ]
                        [ div [ class "color-picker-gradient" ] [] ]
                    ]
                ]


viewBody : Model -> Html Msg
viewBody model =
    if model.appMode == ModeInit then
        div [ class "app-body-empty" ]
            [ viewFileList model
            , viewStatusBadge model
            , viewBodyOverlay model
            ]

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


viewFileList : Model -> Html Msg
viewFileList model =
    let
        isBusy =
            model.loadState == Loading
    in
    div [ class "file-list" ]
        ([ button [ class "file-entry file-entry-browse", onClick PickFile, disabled isBusy ]
            [ text "Browse…" ]
         ]
            ++ (if List.isEmpty model.pdfFiles then
                    [ span [ class "file-list-empty" ] [ text "No files in in/" ] ]

                else
                    List.map
                        (\f ->
                            button
                                [ class "file-entry"
                                , onClick (LoadFile f.path)
                                , disabled isBusy
                                ]
                                [ text f.name ]
                        )
                        model.pdfFiles
               )
        )


viewBodyOverlay : Model -> Html Msg
viewBodyOverlay model =
    let
        msg =
            if model.loadState == Loading then
                Just "Parsing PDF\u{2026}"

            else if model.generateState == Compositing then
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

        isLoadingPdf =
            model.loadState == Loading

        isBusy =
            isLoadingPdf || model.recomputing || model.exporting

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
    div [ class "left-sidebar" ]
        [ span [ class "app-title" ] [ text "House Puzzle" ]
        , div [ class "sidebar-nav" ]
            [ button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModeInit )
                    , ( "loading", isLoadingPdf )
                    ]
                , disabled (isBusy || isGenerating)
                , onClick Reset
                ]
                [ text
                    (if isLoadingPdf then
                        "Loading\u{2026}"

                     else if hasFile then
                        "Reset"

                     else
                        "Start"
                    )
                ]
            , span [ class "mode-sep" ] [ text "\u{2193}" ]
            , button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModeGenerate )
                    , ( "loading", isGenerating )
                    ]
                , disabled (not isLoaded || isBusy || isGenerating)
                , onClick (SetAppMode ModeGenerate)
                ]
                [ text
                    (if isGenerating then
                        "Importing\u{2026}"

                     else
                        "Import"
                    )
                ]
            , span [ class "mode-sep" ] [ text "\u{2193}" ]
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
            , span [ class "mode-sep" ] [ text "\u{2195}" ]
            , button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModeBlueprint )
                    ]
                , disabled (not isGenerated || isBusy || isGenerating)
                , onClick (SetAppMode ModeBlueprint)
                ]
                [ text "Blueprint" ]
            , span [ class "mode-sep" ] [ text "\u{2195}" ]
            , button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModeGroups )
                    ]
                , disabled (not isGenerated || isBusy || isGenerating)
                , onClick (SetAppMode ModeGroups)
                ]
                [ text "Groups" ]
            , span [ class "mode-sep" ] [ text "\u{2193}" ]
            , button
                [ classList
                    [ ( "mode-btn", True )
                    , ( "active", model.appMode == ModeWaves )
                    ]
                , disabled (not isGenerated || isBusy || isGenerating)
                , onClick (SetAppMode ModeWaves)
                ]
                [ text "Waves" ]
            , span [ class "mode-sep" ] [ text "\u{2193}" ]
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
        , div [ class "undo-redo-bar sidebar-nav", style "flex-direction" "row", style "margin-top" "12px" ]
            [ button
                [ classList [ ( "mode-btn", True ), ( "undo-btn", True ) ]
                , style "flex" "1"
                , style "width" "auto"
                , disabled (List.isEmpty model.undoHistory)
                , onClick Undo
                , title "Undo (Ctrl+Z)"
                ]
                [ text "↩" ]
            , button
                [ classList [ ( "mode-btn", True ), ( "redo-btn", True ) ]
                , style "flex" "1"
                , style "width" "auto"
                , disabled (List.isEmpty model.redoHistory)
                , onClick Redo
                , title "Redo (Ctrl+Shift+Z)"
                ]
                [ text "↪" ]
            ]
        , span [ class "version-tag" ] [ text model.appVersion ]
        ]


viewZoomSlider : Model -> Html Msg
viewZoomSlider model =
    let
        pct =
            round (model.zoomLevel * 100)

        label =
            String.fromInt pct ++ "%"
    in
    div [ class "zoom-slider-bar" ]
        [ span [ class "zoom-icon" ] [ text "+" ]
        , div [ class "zoom-slider-wrap" ]
            [ input
                [ type_ "range"
                , class "zoom-slider"
                , Html.Attributes.list "zoom-ticks"
                , Html.Attributes.min "0.25"
                , Html.Attributes.max "4.0"
                , Html.Attributes.step "0.05"
                , value (String.fromFloat model.zoomLevel)
                , onInput (\s -> Maybe.withDefault NoOp (Maybe.map SetZoomLevel (String.toFloat s)))
                , onMouseEnter (SetZoomGridActive True)
                , onMouseLeave (SetZoomGridActive False)
                ]
                []
            , Html.node "datalist" [ id "zoom-ticks" ]
                [ Html.option [ value "1" ] [] ]
            , button
                [ class "zoom-notch-label"
                , onClick (SetZoomLevel 1.0)
                ]
                [ text "100%" ]
            ]
        , span [ class "zoom-icon" ] [ text "−" ]
        , span [ class "zoom-val" ] [ text label ]
        ]


viewCanvasCol : Model -> LoadResponse -> Html Msg
viewCanvasCol model response =
    div [ class "canvas-col" ]
        [ div [ class "canvas-house-wrap" ]
            [ div [ class "canvas-area", id "house-scroll" ]
                [ div [ class "canvas-spacer" ] []
                , viewMainSvg response model
                , if model.recomputing then
                    div [ class "canvas-spinner-overlay" ] [ div [ class "canvas-spinner" ] [] ]

                  else
                    text ""
                ]
            , viewZoomSlider model
            ]
        ]


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
        [ div
            [ class "wave-tray-bg"
            , preventDefaultOn "wheel"
                (D.map2 (\dx dy -> ( ScrollTrayBy (if dx /= 0 then dx else dy), True ))
                    (D.field "deltaX" D.float)
                    (D.field "deltaY" D.float)
                )
            ]
            []
        , div [ class "wave-tray-scroll", id "wave-tray-scroll" ]
            (let
                displays =
                    toPieceDisplays model.groups activeWavePieceIds
                        |> List.indexedMap (\i display -> ( i + 1, display ))

                thumbs =
                    List.concatMap
                        (\( pos, display ) ->
                            let
                                repId =
                                    case display of
                                        SinglePiece pid -> pid
                                        GroupedPiece pid _ -> pid

                                showMarker =
                                    not isLocked && model.draggingPieceId /= Nothing && model.dragInsertBeforeId == Just repId

                                marker =
                                    if showMarker then
                                        [ div [ class "drag-insert-marker-v" ] [] ]

                                    else
                                        []

                                thumb =
                                    case model.pieces |> List.filter (\p -> p.id == repId) |> List.head of
                                        Just piece ->
                                            let
                                                groupCount =
                                                    case display of
                                                        SinglePiece _ -> Nothing
                                                        GroupedPiece _ allIds -> Just (List.length allIds)
                                            in
                                            [ viewWaveTrayThumb piece isLocked model.svgScale model.hoveredPieceId model.pieceGeneration model.showNumbers pos groupCount ]

                                        Nothing ->
                                            []
                            in
                            marker ++ thumb
                        )
                        displays

                endMarker =
                    if not isLocked && model.draggingPieceId /= Nothing && model.dragInsertBeforeId == Nothing && model.dragOverWaveId == Just activeWaveId then
                        [ div [ class "drag-insert-marker-v" ] [] ]

                    else
                        []
             in
             thumbs ++ endMarker
            )
        ]


-- scale: computed from viewport height and SVG natural height (stored in model.svgScale).
-- Produces exact px dimensions matching how the piece appears in the house view.
viewWaveTrayThumb : Piece -> Bool -> Float -> Maybe String -> Int -> Bool -> Int -> Maybe Int -> Html Msg
viewWaveTrayThumb piece isLocked scale hoveredId generation showNum pos maybeGroupN =
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
        [ img [ src (cacheBust piece.imgUrl generation) ] []
        , if showNum then
            div [ class "tray-thumb-num" ] [ text (String.fromInt pos) ]
          else
            text ""
        , case maybeGroupN of
            Just n ->
                div [ class "group-xn-badge group-xn-badge-bottom" ] [ text ("x" ++ String.fromInt n) ]
            Nothing ->
                text ""
        ]


viewToolsCol : Model -> LoadResponse -> Html Msg
viewToolsCol model response =
    div [ class "tools-col" ]
        [ case model.appMode of
            ModeInit ->
                text ""

            ModeGenerate ->
                viewGenerateTools model response

            ModePieces ->
                viewPiecesTools model

            ModeBlueprint ->
                viewBlueprintTools model

            ModeGroups ->
                viewGroupsTools model

            ModeWaves ->
                viewWavesTools model

            ModeExport ->
                viewExportTools model
        ]


viewGenerateTools : Model -> LoadResponse -> Html Msg
viewGenerateTools model response =
    let
        isLoaded =
            case model.loadState of
                Loaded _ -> True
                _ -> False

        isBusy =
            model.loadState == Loading || model.recomputing || model.exporting

        isGenerating =
            model.generateState == Compositing

        hasLights =
            response.lightsUrl /= Nothing
    in
    div [ class "tools-pane" ]
        [ viewTogglesBox [ viewCheckboxLights model, viewCheckboxGrid model ]
        , viewStatusBadge model
        , viewSectionTitle "Import"
        , div [ class "param-group" ]
            [ label [] [ text "Target Pieces ", span [ class "value" ] [ text (String.fromInt model.targetCount) ] ]
            , input [ type_ "range", Html.Attributes.min "5", Html.Attributes.max "181", value (String.fromInt model.targetCount), onInput SetTargetCount ] []
            ]
        , div [ class "param-group" ]
            [ label [] [ text "Min. Common Border Length ", span [ class "value" ] [ text (String.fromInt model.minBorder) ], text "px" ]
            , input [ type_ "range", Html.Attributes.min "0", Html.Attributes.max "50", value (String.fromInt model.minBorder), onInput SetMinBorder ] []
            ]
        , h2 [] [ text "Import" ]
        , viewImportStats response
        , h2 [] [ text "Puzzle" ]
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
            let
                selectedPiece =
                    model.selectedPieceId
                        |> Maybe.andThen (\pid -> model.pieces |> List.filter (\p -> p.id == pid) |> List.head)
            in
            [ viewTogglesBox [ viewCheckboxLights model, viewCheckboxGrid model, viewCheckboxOutlines model ]
            , viewSectionTitle "Edit Pieces"
            , case selectedPiece of
                Just piece ->
                    div [ class "piece-info" ]
                        [ div [ class "piece-info-row" ] [ text ("Piece ID: " ++ piece.id) ]
                        , div [ class "piece-info-row" ] [ text ("Bricks: " ++ String.fromInt (List.length piece.brickIds)) ]
                        , div [ class "piece-info-row" ]
                            [ text ("Brick IDs: " ++ String.join ", " piece.brickIds) ]
                        , button
                            [ class "primary"
                            , onClick StartEdit
                            , disabled model.recomputing
                            ]
                            [ text "Edit Piece" ]
                        ]

                Nothing ->
                    div [ class "piece-info-empty" ] [ text "Click a piece to select" ]
            ]
        )


viewWavePieceInfoBox : Model -> Html Msg
viewWavePieceInfoBox model =
    let
        focusId =
            case model.hoveredPieceId of
                Just pid ->
                    Just pid

                Nothing ->
                    model.selectedPieceId

        piecePositions =
            model.waves
                |> List.concatMap (\wv -> List.indexedMap (\i pid -> ( pid, i + 1 )) wv.pieceIds)
                |> Dict.fromList

        waveOfPiece pid =
            model.waves
                |> List.indexedMap (\i wv -> ( i + 1, wv ))
                |> List.filter (\( _, wv ) -> List.member pid wv.pieceIds)
                |> List.head
                |> Maybe.map Tuple.first
    in
    case focusId of
        Just pid ->
            let
                maybePiece =
                    model.pieces |> List.filter (\p -> p.id == pid) |> List.head

                posLabel =
                    case Dict.get pid piecePositions of
                        Just pos ->
                            case waveOfPiece pid of
                                Just waveNum ->
                                    "Wave " ++ String.fromInt waveNum ++ ", pos " ++ String.fromInt pos

                                Nothing ->
                                    "pos " ++ String.fromInt pos

                        Nothing ->
                            "Unassigned"
            in
            div [ class "stats" ]
                (case maybePiece of
                    Just piece ->
                        [ div [ class "row" ]
                            [ span [] [ text "Position" ]
                            , span [ class "val" ] [ text posLabel ]
                            ]
                        , div [ class "row" ]
                            [ span [] [ text "Piece ID" ]
                            , span [ class "val" ] [ text pid ]
                            ]
                        , div [ class "row" ]
                            [ span [] [ text "Bricks" ]
                            , span [ class "val" ] [ text (String.fromInt (List.length piece.brickIds)) ]
                            ]
                        , div [ class "row" ]
                            [ span [] [ text "Brick IDs" ]
                            , span [ class "val" ] [ text (String.join ", " piece.brickIds) ]
                            ]
                        ]

                    Nothing ->
                        [ div [ class "row" ]
                            [ span [] [ text "Position" ]
                            , span [ class "val" ] [ text posLabel ]
                            ]
                        , div [ class "row" ]
                            [ span [] [ text "Piece ID" ]
                            , span [ class "val" ] [ text pid ]
                            ]
                        ]
                )

        Nothing ->
            div [ class "stats" ]
                [ div [ class "row" ]
                    [ span [ style "color" "#aaa", style "font-style" "italic" ] [ text "Hover a piece to inspect" ] ]
                ]


viewTogglesBox : List (Html Msg) -> Html Msg
viewTogglesBox children =
    div [ class "toggles-box" ] children


viewSectionTitle : String -> Html Msg
viewSectionTitle title =
    h3 [ class "section-title" ] [ text title ]


viewCheckboxLights : Model -> Html Msg
viewCheckboxLights model =
    div [ class "checkbox-group" ]
        [ input [ type_ "checkbox", id "cbLights", checked model.showLights, onCheck ToggleLights ] []
        , label [ for "cbLights" ] [ text "Show lights" ]
        ]


viewCheckboxGrid : Model -> Html Msg
viewCheckboxGrid model =
    div [ class "checkbox-group" ]
        [ input [ type_ "checkbox", id "cbGrid", checked model.showGrid, onCheck ToggleGrid ] []
        , label [ for "cbGrid" ] [ text "Show grid" ]
        , viewGridColorSwatch model
        ]


viewCheckboxOutlines : Model -> Html Msg
viewCheckboxOutlines model =
    div [ class "checkbox-group" ]
        [ input [ type_ "checkbox", id "cbOutlines", checked model.showOutlines, onCheck ToggleOutlines ] []
        , label [ for "cbOutlines" ] [ text "Show piece outlines" ]
        , span
            [ class "wave-swatch wave-swatch-sm"
            , style "background-color" (waveColor model.outlineHue 1.0)
            , stopPropagationOn "mousedown"
                (D.map2 (\mx my -> ( StartColorPick OutlineColorTarget mx my, True ))
                    (D.field "clientX" D.float)
                    (D.field "clientY" D.float)
                )
            , title "Pick outline color"
            ]
            []
        ]


viewCheckboxNumbers : Model -> Html Msg
viewCheckboxNumbers model =
    div [ class "checkbox-group" ]
        [ input [ type_ "checkbox", id "cbNumbers", checked model.showNumbers, onCheck ToggleNumbers ] []
        , label [ for "cbNumbers" ] [ text "Show position numbers" ]
        ]


viewCheckboxGroupOverlay : Model -> Html Msg
viewCheckboxGroupOverlay model =
    div [ class "checkbox-group" ]
        [ input [ type_ "checkbox", id "cbGroupOverlay", checked model.showGroupOverlay, onCheck ToggleGroupOverlay ] []
        , label [ for "cbGroupOverlay" ] [ text "Show overlay" ]
        ]


viewCheckboxWaveOverlay : Model -> Html Msg
viewCheckboxWaveOverlay model =
    div [ class "checkbox-group" ]
        [ input [ type_ "checkbox", id "cbWaveOverlay", checked model.showWaveOverlay, onCheck ToggleWaveOverlay ] []
        , label [ for "cbWaveOverlay" ] [ text "Show wave overlays" ]
        ]


viewGridColorSwatch : Model -> Html Msg
viewGridColorSwatch model =
    span
        [ class "wave-swatch wave-swatch-sm"
        , style "background-color" (waveColor model.gridHue 1.0)
        , stopPropagationOn "mousedown"
            (D.map2 (\mx my -> ( StartColorPick GridColorTarget mx my, True ))
                (D.field "clientX" D.float)
                (D.field "clientY" D.float)
            )
        , title "Pick grid color"
        ]
        []


viewBlueprintTools : Model -> Html Msg
viewBlueprintTools model =
    div [ class "tools-pane" ]
        [ viewTogglesBox [ viewCheckboxLights model, viewCheckboxGrid model ]
        , viewSectionTitle "Blueprint"
        ]


-- ── Groups helpers ──────────────────────────────────────────────────────────


toPieceDisplays : List Group -> List String -> List PieceDisplay
toPieceDisplays groups pieceIds =
    let
        go remaining seen acc =
            case remaining of
                [] ->
                    List.reverse acc

                pid :: rest ->
                    case groups |> List.filter (\g -> not (List.isEmpty g.pieceIds) && List.member pid g.pieceIds) |> List.head of
                        Just g ->
                            if List.member g.id seen then
                                go rest seen acc

                            else
                                go rest (g.id :: seen) (GroupedPiece (Maybe.withDefault pid (List.head g.pieceIds)) g.pieceIds :: acc)

                        Nothing ->
                            go rest seen (SinglePiece pid :: acc)
    in
    go pieceIds [] []


-- ── Groups tools pane ────────────────────────────────────────────────────────


viewGroupsTools : Model -> Html Msg
viewGroupsTools model =
    let
        assignedIds =
            List.concatMap .pieceIds model.groups

        totalPieces =
            List.length model.pieces

        assignedCount =
            List.length assignedIds

        unassignedPieces =
            List.filter (\p -> not (List.member p.id assignedIds)) model.pieces
    in
    div [ class "tools-pane waves-tools" ]
        [ viewTogglesBox [ viewCheckboxLights model, viewCheckboxGrid model, viewCheckboxOutlines model, viewCheckboxGroupOverlay model ]
        , div [ class "waves-header" ]
            [ viewSectionTitle "Groups"
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
            [ button [ onClick AddGroup ] [ text "New group" ]
            ]
        , div [ class "waves-body" ]
            (List.map (viewGroupRow model model.groups) model.groups
                ++ [ viewGroupUnassignedRow model unassignedPieces ]
            )
        ]


viewGroupRow : Model -> List Group -> Group -> Html Msg
viewGroupRow model allGroups group =
    let
        isSelected =
            model.selectedGroupId == Just group.id

        groupCount =
            List.length allGroups

        swatchColor =
            waveColor group.hue 0.85
    in
    div
        [ classList
            [ ( "wave-row", True )
            , ( "selected", isSelected )
            , ( "drag-over", model.dragOverGroupId == Just (Just group.id) )
            ]
        , preventDefaultOn "dragover" (D.succeed ( NoOp, True ))
        , on "dragenter" (D.succeed (DragEnterGroup (Just group.id)))
        , on "drop" (D.succeed (DropOnGroup (Just group.id)))
        ]
        [ div
            [ class "wave-row-header"
            , onClick
                (if isSelected && groupCount > 1 then
                    SelectGroup Nothing
                 else
                    SelectGroup (Just group.id)
                )
            ]
            [ span
                [ classList [ ( "wave-lock", True ), ( "locked", group.locked ) ]
                , stopPropagationOn "click" (D.succeed ( ToggleGroupLock group.id, True ))
                , title (if group.locked then "Unlock group" else "Lock group")
                ]
                [ if group.locked then iconLockClosed else iconLockOpen ]
            , span
                [ class "wave-swatch"
                , style "background-color" swatchColor
                , stopPropagationOn "mousedown"
                    (D.map2 (\mx my -> ( StartColorPick (GroupColorTarget group.id) mx my, True ))
                        (D.field "clientX" D.float)
                        (D.field "clientY" D.float)
                    )
                , title "Pick color"
                ]
                []
            , span [ class "wave-piece-count-label", style "color" (waveColor group.hue 1.0) ]
                [ text (String.fromInt (List.length group.pieceIds) ++ " pcs") ]
            , span [ class "wave-name-label" ] [ text group.name ]
            , span [ class "wave-row-spacer" ] []
            , span [ class "wave-actions" ]
                [ button
                    [ stopPropagationOn "click" (D.succeed ( RemoveGroup group.id, True ))
                    , disabled (groupCount <= 1)
                    , title "Delete group"
                    ]
                    [ text "\u{2715}" ]
                ]
            ]
        , div [ class "wave-pieces" ]
            (List.filterMap
                (\pid ->
                    model.pieces
                        |> List.filter (\p -> p.id == pid)
                        |> List.head
                        |> Maybe.map (\piece -> viewPieceThumb (Just ( group.id, pid )) False model.hoveredPieceId pid (cacheBust piece.imgUrl model.pieceGeneration) Nothing)
                )
                group.pieceIds
            )
        ]


viewGroupUnassignedRow : Model -> List Piece -> Html Msg
viewGroupUnassignedRow model unassignedPieces =
    if List.isEmpty model.pieces then
        text ""

    else
        div
            [ classList
                [ ( "wave-row", True )
                , ( "drag-over", model.dragOverGroupId == Just Nothing )
                ]
            , preventDefaultOn "dragover" (D.succeed ( NoOp, True ))
            , on "dragenter" (D.succeed (DragEnterGroup Nothing))
            , on "drop" (D.succeed (DropOnGroup Nothing))
            ]
            [ div [ class "wave-row-header" ]
                [ span [ class "wave-label unassigned-label" ] [ text "Unassigned" ]
                , span [ class "wave-piece-count" ]
                    [ text (String.fromInt (List.length unassignedPieces) ++ " pcs") ]
                ]
            , div [ class "wave-pieces" ]
                (List.map
                    (\p -> viewPieceThumb Nothing False model.hoveredPieceId p.id (p.imgUrl ++ "?v=" ++ String.fromInt model.pieceGeneration) Nothing)
                    unassignedPieces
                )
            ]


-- ── Waves tools pane ─────────────────────────────────────────────────────────


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
        [ viewTogglesBox [ viewCheckboxLights model, viewCheckboxGrid model, viewCheckboxOutlines model, viewCheckboxWaveOverlay model, viewCheckboxNumbers model ]
        , div [ class "waves-header" ]
            [ viewSectionTitle "Waves"
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
            [ button [ onClick AddWave ] [ text "New wave" ]
            ]
        , div [ class "waves-body" ]
            (List.map (viewWaveRow model model.waves) model.waves
                ++ [ viewUnassignedRow model unassignedPieces ]
            )
        , div [ class "tools-divider" ] []
        , viewWavePieceInfoBox model
        ]


locations : List String
locations =
    [ "Tutorial", "Rome", "Athens", "Amsterdam", "Paris", "Palermo", "Venice", "Frankfurt", "New York", "Prague" ]


viewExportTools : Model -> Html Msg
viewExportTools model =
    let
        assignedIds =
            model.waves |> List.concatMap .pieceIds

        hasUnassigned =
            List.any (\p -> not (List.member p.id assignedIds)) model.pieces
    in
    div [ class "tools-pane" ]
        [ viewTogglesBox [ viewCheckboxLights model, viewCheckboxGrid model, viewCheckboxOutlines model, viewCheckboxWaveOverlay model, viewCheckboxNumbers model ]
        , viewSectionTitle "Export"
        , div [ class "field-row" ]
            [ label [] [ text "Location" ]
            , Html.select
                [ onInput SetExportLocation ]
                (List.map
                    (\loc ->
                        Html.option
                            [ value loc
                            , Html.Attributes.selected (loc == model.exportLocation)
                            ]
                            [ text loc ]
                    )
                    locations
                )
            ]
        , div [ class "field-row" ]
            [ label [] [ text "House name" ]
            , input
                [ type_ "text"
                , value model.exportHouseName
                , onInput SetExportHouseName
                ]
                []
            ]
        , div [ class "field-row" ]
            [ label [] [ text "Position in location" ]
            , input
                [ type_ "number"
                , value model.exportPosition
                , onInput SetExportPosition
                , Html.Attributes.min "0"
                , Html.Attributes.step "1"
                ]
                []
            ]
        , div [ class "field-row" ]
            [ label [] [ text "Spacing (units)" ]
            , input
                [ type_ "number"
                , value model.exportSpacing
                , onInput SetExportSpacing
                , Html.Attributes.min "0"
                , Html.Attributes.step "0.5"
                ]
                []
            ]
        , button
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

        showPieceImages =
            (model.appMode == ModePieces || model.appMode == ModeGroups || model.appMode == ModeWaves || model.appMode == ModeExport) && isGenerated && not (List.isEmpty model.pieces)

        showComposite =
            response.hasComposite && (not isGenerated || model.appMode == ModeGenerate)

        -- Pieces hidden by invisible waves (only in Waves section)
        hiddenPieceIds =
            if model.appMode == ModeWaves then
                model.waves
                    |> List.filter (\wv -> not wv.visible)
                    |> List.concatMap .pieceIds
            else
                []

        visiblePieces =
            let
                filtered =
                    List.filter (\p -> not (List.member p.id hiddenPieceIds)) model.pieces
            in
            case model.draggingPieceId of
                Just dragId ->
                    List.filter (\p -> p.id /= dragId) filtered
                        ++ List.filter (\p -> p.id == dragId) filtered

                Nothing ->
                    filtered

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
                        , attribute "href" response.compositeUrl
                        ]
                        []
                    ]

                else
                    []

            else if showPieceImages then
                List.map (viewPieceImage model.pieceGeneration) visiblePieces

            else if showComposite then
                [ Svg.image
                    [ SA.x "0"
                    , SA.y "0"
                    , SA.width w
                    , SA.height h
                    , attribute "href" response.compositeUrl
                    ]
                    []
                ]

            else
                -- Blueprint or pieces mode post-gen: hide bricks, piece polygons/images show through
                []

        -- Background image layer — shown in Blueprint and Waves modes when blueprintBgUrl is available.
        -- Sits beneath piece outlines and piece images so bricks render on top of it.
        bgImageLayer =
            case response.blueprintBgUrl of
                Just url ->
                    if model.appMode == ModeBlueprint || model.appMode == ModeWaves then
                        [ Svg.image
                            [ SA.x "0"
                            , SA.y "0"
                            , SA.width w
                            , SA.height h
                            , attribute "href" url
                            , SA.style "pointer-events: none;"
                            ]
                            []
                        ]

                    else
                        []

                Nothing ->
                    []

        -- Lights overlay (toggleable, shown when showLights is True and lightsUrl is available)
        lightsLayer =
            case ( model.showLights, response.lightsUrl ) of
                ( True, Just url ) ->
                    [ Svg.image
                        [ SA.x "0"
                        , SA.y "0"
                        , SA.width w
                        , SA.height h
                        , attribute "href" url
                        , SA.style "pointer-events: none;"
                        ]
                        []
                    ]

                _ ->
                    []

        -- Outlines PNG overlay (pre-gen only, shows vector brick shapes from PDF)
        outlinesPngLayer =
            if not model.editMode && not isGenerated then
                [ Svg.image
                    [ SA.x "0"
                    , SA.y "0"
                    , SA.width w
                    , SA.height h
                    , attribute "href" response.outlinesUrl
                    , SA.style "pointer-events: none;"
                    ]
                    []
                ]

            else
                []

        -- Composite brick hover overlays (pre-gen only)
        compositeOverlays =
            if showComposite then
                List.map viewBrickOverlay response.bricks

            else
                []

        -- Edit mode: green polygon overlay for the piece being edited
        editActivePieceOverlay =
            if model.editMode then
                case model.selectedPieceId of
                    Nothing ->
                        []

                    Just pid ->
                        case List.filter (\p -> p.id == pid) model.pieces |> List.head of
                            Nothing ->
                                []

                            Just piece ->
                                if List.isEmpty piece.polygon then
                                    []

                                else
                                    let
                                        pointsAttr =
                                            piece.polygon
                                                |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                                                |> String.join " "
                                    in
                                    [ Svg.polygon
                                        [ SA.points pointsAttr
                                        , SA.fill "rgba(40,180,80,0.25)"
                                        , SA.stroke "rgba(40,180,80,0.9)"
                                        , SA.strokeWidth "3"
                                        , SA.strokeLinejoin "round"
                                        , attribute "vector-effect" "non-scaling-stroke"
                                        , SA.style "pointer-events: none;"
                                        ]
                                        []
                                    ]

            else
                []

        -- Edit mode: brick overlays for context-sensitive clicking
        editOverlays =
            if model.editMode then
                let
                    brickToPiece =
                        model.pieces
                            |> List.concatMap (\p -> List.map (\bid -> ( bid, p.id )) p.brickIds)
                            |> Dict.fromList
                in
                List.map (viewBrickEditOverlay model.editBrickIds brickToPiece model.hoveredPieceId) response.bricks

            else
                []

        effectiveScale =
            model.svgScale * model.zoomLevel

        -- Grid lines
        gridLayer =
            if (not model.editMode) && (model.showGrid || model.zoomGridActive) then
                viewGrid cw ch (waveColor model.gridHue 1.0) model.houseUnitsHigh

            else
                []

        -- Piece outlines (post-gen, pieces/waves mode only; always shown in edit mode so blue outlines stay visible)
        outlineLayer =
            if isGenerated && (model.editMode || (model.showOutlines && (model.appMode == ModePieces || model.appMode == ModeGroups || model.appMode == ModeWaves || model.appMode == ModeExport))) then
                List.map (viewPieceOutline (waveColor model.outlineHue 1.0)) visiblePieces

            else
                []

        -- Green hover highlights rendered on top of outlines (edit mode only)
        greenHoverLayer =
            if model.editMode then
                let
                    brickToPiece =
                        model.pieces
                            |> List.concatMap (\p -> List.map (\bid -> ( bid, p.id )) p.brickIds)
                            |> Dict.fromList
                in
                List.concatMap (viewGreenHoverOverlay model.editBrickIds brickToPiece model.hoveredPieceId model.hoveredBrickId) response.bricks

            else
                []

        -- Green polygon outline for hovered foreign piece (whole-piece outline, not per-brick)
        greenPieceOutlineLayer =
            if model.editMode then
                case model.hoveredPieceId of
                    Nothing ->
                        []

                    Just pid ->
                        if Just pid == model.selectedPieceId then
                            []

                        else
                            case List.filter (\p -> p.id == pid) model.pieces |> List.head of
                                Nothing ->
                                    []

                                Just piece ->
                                    if List.isEmpty piece.polygon then
                                        []

                                    else
                                        let
                                            pointsAttr =
                                                piece.polygon
                                                    |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                                                    |> String.join " "
                                        in
                                        [ Svg.polygon
                                            [ SA.points pointsAttr
                                            , SA.fill "none"
                                            , SA.stroke "rgba(40,180,80,0.9)"
                                            , SA.strokeWidth "3"
                                            , SA.strokeLinejoin "round"
                                            , attribute "vector-effect" "non-scaling-stroke"
                                            , SA.style "pointer-events: none;"
                                            ]
                                            []
                                        ]

            else
                []

        -- Piece interaction overlays (post-gen, not in edit)
        effectiveHoverId =
            if model.draggingPieceId /= Nothing then
                model.draggingPieceId

            else
                model.hoveredPieceId

        isLassoing =
            model.lasso /= Nothing

        showOverlayFill =
            (model.appMode == ModeGroups && model.showGroupOverlay)
                || (model.appMode == ModeWaves && model.showWaveOverlay)
                || (model.appMode == ModeExport && model.showWaveOverlay)

        pieceOverlays =
            if (not model.editMode) && isGenerated then
                List.map (viewPieceOverlay model.appMode effectiveHoverId model.selectedPieceId model.selectedWaveId model.waves model.groups model.selectedGroupId isLassoing showOverlayFill) visiblePieces

            else
                []

        -- Piece position number labels (post-gen, not in edit, when showNumbers is on)
        -- Groups count as one slot: all pieces in a group share the same position number.
        piecePositions =
            model.waves
                |> List.concatMap
                    (\wv ->
                        toPieceDisplays model.groups wv.pieceIds
                            |> List.indexedMap
                                (\i display ->
                                    case display of
                                        SinglePiece pid ->
                                            [ ( pid, i + 1 ) ]

                                        GroupedPiece _ allIds ->
                                            List.map (\pid -> ( pid, i + 1 )) allIds
                                )
                            |> List.concat
                    )
                |> Dict.fromList

        numberLabels =
            if (not model.editMode) && isGenerated && model.showNumbers && (model.appMode == ModePieces || model.appMode == ModeWaves || model.appMode == ModeExport) then
                List.filterMap
                    (\piece ->
                        Dict.get piece.id piecePositions
                            |> Maybe.map (viewPieceNumberLabel piece)
                    )
                    visiblePieces

            else
                []
        -- Decoder: convert offsetX/offsetY (CSS px relative to SVG element) -> SVG coords
        decodeLassoCoords toMsg =
            D.map2 toMsg
                (D.map (\x -> x / effectiveScale - 200) (D.field "offsetX" D.float))
                (D.map (\y -> y / effectiveScale - 10) (D.field "offsetY" D.float))

        -- Transparent background rect to catch lasso mousedown (only in waves mode with wave selected)
        lassoBackdrop =
            if (not model.editMode) && isGenerated && model.selectedWaveId /= Nothing then
                [ Svg.rect
                    [ SA.x "-200"
                    , SA.y "-10"
                    , SA.width (String.fromFloat (cw + 400))
                    , SA.height (String.fromFloat (ch + 20))
                    , SA.fill "transparent"
                    , SA.style "cursor: crosshair;"
                    , on "mousedown" (decodeLassoCoords LassoStart)
                    ]
                    []
                ]

            else
                []

        -- Lasso selection rectangle (shown while dragging)
        lassoRect =
            case model.lasso of
                Nothing ->
                    []

                Just ls ->
                    let
                        rx = Basics.min ls.x0 ls.x1
                        ry = Basics.min ls.y0 ls.y1
                        rw = abs (ls.x1 - ls.x0)
                        rh = abs (ls.y1 - ls.y0)
                    in
                    [ Svg.rect
                        [ SA.x (String.fromFloat rx)
                        , SA.y (String.fromFloat ry)
                        , SA.width (String.fromFloat rw)
                        , SA.height (String.fromFloat rh)
                        , SA.fill "rgba(64,120,255,0.1)"
                        , SA.stroke "rgba(64,120,255,0.8)"
                        , SA.strokeWidth "1.5"
                        , SA.strokeDasharray "4 3"
                        , attribute "vector-effect" "non-scaling-stroke"
                        , SA.style "pointer-events: none;"
                        ]
                        []
                    ]

        -- SVG-level mouse events for lasso drag tracking
        lassoSvgAttrs =
            if isLassoing then
                [ on "mousemove" (decodeLassoCoords LassoMove)
                , on "mouseup" (D.succeed LassoEnd)
                , on "mouseleave" (D.succeed LassoEnd)
                ]

            else
                []
    in
    Svg.svg
        ([ SA.viewBox ("-200 -10 " ++ String.fromFloat (cw + 400) ++ " " ++ String.fromFloat (ch + 20))
         , SA.class "house-svg"
         , SA.width (String.fromFloat ((cw + 400) * effectiveScale))
         , SA.height (String.fromFloat ((ch + 20) * effectiveScale))
         ]
            ++ lassoSvgAttrs
        )
        (if model.editMode then
            [ Svg.g [] baseLayer
            , Svg.g [] editOverlays
            , Svg.g [] outlineLayer
            , Svg.g [] editActivePieceOverlay
            , Svg.g [] greenHoverLayer
            , Svg.g [] greenPieceOutlineLayer
            ]

         else
            [ Svg.g [] bgImageLayer
            , Svg.g [] blueprintLayer
            , Svg.g [] baseLayer
            , Svg.g [] lightsLayer
            , Svg.g [] compositeOverlays
            , Svg.g [] gridLayer
            , Svg.g [] lassoBackdrop
            , Svg.g [] pieceOverlays
            , Svg.g [] outlineLayer
            , Svg.g [] outlinesPngLayer
            , Svg.g [] numberLabels
            , Svg.g [] lassoRect
            ]
        )


viewPieceImage : Int -> Piece -> Svg.Svg Msg
viewPieceImage generation piece =
    Svg.image
        [ SA.x (String.fromFloat piece.x)
        , SA.y (String.fromFloat piece.y)
        , SA.width (String.fromFloat piece.width)
        , SA.height (String.fromFloat piece.height)
        , attribute "href" (cacheBust piece.imgUrl generation)
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
        -- ERROR: no polygon from PDF vector layer — must never happen, all shapes are complex polygons
        Svg.g []
            [ Svg.rect
                [ SA.x (String.fromFloat brick.x)
                , SA.y (String.fromFloat brick.y)
                , SA.width "20"
                , SA.height "20"
                , SA.fill "red"
                , SA.opacity "0.8"
                ]
                []
            , Svg.text_
                [ SA.x (String.fromFloat (brick.x + 2))
                , SA.y (String.fromFloat (brick.y + 14))
                , SA.fontSize "12"
                , SA.fill "white"
                , SA.fontWeight "bold"
                ]
                [ Svg.text ("!" ++ brick.id) ]
            ]

    else
        Svg.polygon
            [ SA.points pointsAttr
            , SA.fill "transparent"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "brick-overlay"
            , onClick (LogBrickClick brick.id)
            ]
            []


viewBrickEditOverlay : List String -> Dict String String -> Maybe String -> Brick -> Svg.Svg Msg
viewBrickEditOverlay editBrickIds brickToPiece hoveredPieceId brick =
    let
        inEdit =
            List.member brick.id editBrickIds

        -- For out-bricks, which piece does this brick belong to?
        outBrickPieceId =
            if inEdit then
                Nothing
            else
                Dict.get brick.id brickToPiece

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

        clickMsg =
            if inEdit then
                -- Remove from edited piece (unless it's the last brick)
                if List.length editBrickIds <= 1 then
                    LogBrickClick brick.id
                else
                    RemoveBrickFromEdit brick.id
            else
                -- Merge the piece this brick belongs to into the edited piece
                case Dict.get brick.id brickToPiece of
                    Just pid ->
                        MergePieceIntoEdit pid
                    Nothing ->
                        LogBrickClick brick.id

        -- Mouse events: track hover for both in-edit (brick-level) and out-bricks (piece-level)
        hoverAttrs =
            if inEdit then
                [ onMouseEnter (SetHoveredBrick (Just brick.id))
                , onMouseLeave (SetHoveredBrick Nothing)
                ]
            else
                case outBrickPieceId of
                    Just pid ->
                        [ onMouseEnter (SetHoveredPiece (Just pid))
                        , onMouseLeave (SetHoveredPiece Nothing)
                        ]
                    Nothing ->
                        []
    in
    if List.isEmpty absPoints then
        -- ERROR: no polygon — all bricks must have vector polygons
        Svg.g []
            [ Svg.rect
                [ SA.x (String.fromFloat brick.x)
                , SA.y (String.fromFloat brick.y)
                , SA.width "20"
                , SA.height "20"
                , SA.fill "red"
                , SA.opacity "0.8"
                ]
                []
            , Svg.text_
                [ SA.x (String.fromFloat (brick.x + 2))
                , SA.y (String.fromFloat (brick.y + 14))
                , SA.fontSize "12"
                , SA.fill "white"
                , SA.fontWeight "bold"
                ]
                [ Svg.text ("!" ++ brick.id) ]
            ]

    else
        Svg.polygon
            ([ SA.points pointsAttr
            , SA.class cls
            , attribute "vector-effect" "non-scaling-stroke"
            , onClick clickMsg
            ] ++ hoverAttrs)
            []


-- Green hover overlay for edit mode: renders on top of blue outlines.
-- For in-edit bricks: highlight the individual brick when it is hovered (fill + brick outline).
-- For out-bricks: highlight the brick fill when its piece is the hovered piece.
--   (The whole-piece green outline is handled separately by greenPieceOutlineLayer.)
viewGreenHoverOverlay : List String -> Dict String String -> Maybe String -> Maybe String -> Brick -> List (Svg.Svg Msg)
viewGreenHoverOverlay editBrickIds brickToPiece hoveredPieceId hoveredBrickId brick =
    let
        inEdit =
            List.member brick.id editBrickIds

        shouldHighlight =
            if inEdit then
                hoveredBrickId == Just brick.id
            else
                case Dict.get brick.id brickToPiece of
                    Just pid ->
                        hoveredPieceId == Just pid
                    Nothing ->
                        False
    in
    if not shouldHighlight || List.isEmpty brick.polygon then
        []
    else
        let
            absPoints =
                List.map (\( x, y ) -> ( x + brick.x, y + brick.y )) brick.polygon

            pointsAttr =
                absPoints
                    |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                    |> String.join " "
        in
        if inEdit then
            -- In-edit brick hovered: green fill + green outline around just this brick
            [ Svg.polygon
                [ SA.points pointsAttr
                , SA.fill "rgba(40,180,80,0.3)"
                , SA.stroke "rgba(40,180,80,0.9)"
                , SA.strokeWidth "3"
                , attribute "vector-effect" "non-scaling-stroke"
                , SA.style "pointer-events: none;"
                ]
                []
            ]
        else
            -- Foreign piece brick hovered: green fill only (piece polygon outline handled by greenPieceOutlineLayer)
            [ Svg.polygon
                [ SA.points pointsAttr
                , SA.fill "rgba(40,180,80,0.3)"
                , SA.stroke "none"
                , attribute "vector-effect" "non-scaling-stroke"
                , SA.style "pointer-events: none;"
                ]
                []
            ]


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
            , SA.fill "none"
            , SA.stroke "white"
            , SA.strokeWidth "4"
            , SA.strokeLinejoin "round"
            , attribute "stroke-linecap" "round"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "brick-path"
            ]
            []


viewPieceOutline : String -> Piece -> Svg.Svg Msg
viewPieceOutline color piece =
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
            , SA.stroke color
            , SA.strokeWidth "3"
            , SA.strokeLinejoin "round"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "piece-outline"
            , attribute "pointer-events" "none"
            ]
            []


viewPieceNumberLabel : Piece -> Int -> Svg.Svg Msg
viewPieceNumberLabel piece pos =
    let
        -- Pick brick whose center is most interior to the piece bbox
        -- (largest min-distance to any piece edge)
        brickScore b =
            let
                bcx = b.x + b.width / 2
                bcy = b.y + b.height / 2
                dl = bcx - piece.x
                dr = (piece.x + piece.width) - bcx
                dt = bcy - piece.y
                db = (piece.y + piece.height) - bcy
            in
            Basics.min (Basics.min dl dr) (Basics.min dt db)

        bestBrick =
            piece.bricks
                |> List.sortBy (\b -> -(brickScore b))
                |> List.head

        ( rawCx, rawCy ) =
            case bestBrick of
                Just b ->
                    ( b.x + b.width / 2, b.y + b.height / 2 )

                Nothing ->
                    ( piece.x + piece.width / 2, piece.y + piece.height / 2 )

        -- Smaller font for small pieces
        minDim = Basics.min piece.width piece.height
        ( fontSizeNum, fontSizeStr ) =
            if minDim < 20 then ( 14, "14" )
            else if minDim < 35 then ( 18, "18" )
            else ( 25, "25" )

        -- Clamp to piece bbox — half font size margin so text never overhangs
        halfFont = fontSizeNum / 2 + 2
        cx = Basics.max (piece.x + halfFont) (Basics.min (piece.x + piece.width - halfFont) rawCx)
        cy = Basics.max (piece.y + halfFont) (Basics.min (piece.y + piece.height - halfFont) rawCy)

        label =
            String.fromInt pos
    in
    Svg.g [ SA.class "piece-number-label", attribute "pointer-events" "none" ]
        [ Svg.text_
            [ SA.x (String.fromFloat cx)
            , SA.y (String.fromFloat cy)
            , SA.textAnchor "middle"
            , SA.dominantBaseline "central"
            , SA.class "piece-num-shadow"
            , SA.fontSize fontSizeStr
            ]
            [ Svg.text label ]
        , Svg.text_
            [ SA.x (String.fromFloat cx)
            , SA.y (String.fromFloat cy)
            , SA.textAnchor "middle"
            , SA.dominantBaseline "central"
            , SA.class "piece-num-text"
            , SA.fontSize fontSizeStr
            ]
            [ Svg.text label ]
        ]


defaultHue : Int -> Float
defaultHue idx =
    case modBy 7 idx of
        0 -> 0
        1 -> 120
        2 -> 40
        3 -> 270
        4 -> 20
        5 -> 180
        _ -> 310


hslToRgb : Float -> ( Int, Int, Int )
hslToRgb hue =
    let
        h = hue / 60
        i = floor h
        f = h - toFloat i
        q = round (255 * (1 - f))
        p = round (255 * f)
    in
    case modBy 6 i of
        0 -> ( 255, p, 0 )
        1 -> ( q, 255, 0 )
        2 -> ( 0, 255, p )
        3 -> ( 0, q, 255 )
        4 -> ( p, 0, 255 )
        _ -> ( 255, 0, q )


waveColor : Float -> Float -> String
waveColor hue opacity =
    if hue < -1.5 then
        -- Special: black
        "rgba(0,0,0," ++ String.fromFloat opacity ++ ")"
    else if hue < -0.5 then
        -- Special: white
        "rgba(255,255,255," ++ String.fromFloat opacity ++ ")"
    else
        let
            ( r, g, b ) = hslToRgb hue
        in
        "rgba(" ++ String.fromInt r ++ "," ++ String.fromInt g ++ "," ++ String.fromInt b ++ "," ++ String.fromFloat opacity ++ ")"


viewPieceOverlay : AppMode -> Maybe String -> Maybe String -> Maybe Int -> List Wave -> List Group -> Maybe Int -> Bool -> Bool -> Piece -> Svg.Svg Msg
viewPieceOverlay appMode hoveredId selectedId selectedWaveId waves groups selectedGroupId isLassoing showOverlayFill piece =
    let
        inWaveAssign =
            appMode == ModeWaves && selectedWaveId /= Nothing

        inGroupAssign =
            appMode == ModeGroups && selectedGroupId /= Nothing

        isHov =
            hoveredId == Just piece.id

        isSel =
            not inWaveAssign && not inGroupAssign && selectedId == Just piece.id

        maybeWave =
            waves
                |> List.filter (\w -> w.visible && List.member piece.id w.pieceIds)
                |> List.head

        maybeGroup =
            groups
                |> List.filter (\g -> List.member piece.id g.pieceIds)
                |> List.head

        fillStyle =
            if appMode == ModeGroups then
                case maybeGroup of
                    Just g ->
                        if showOverlayFill then
                            let eff = if isHov then Basics.min 1.0 (0.35 + 0.15) else 0.35
                            in "fill: " ++ waveColor g.hue eff ++ ";"
                        else if isHov then "fill: rgba(64,120,255,0.2);"
                        else "fill: transparent;"
                    Nothing ->
                        if isHov then "fill: rgba(64,120,255,0.2);"
                        else "fill: transparent;"

            else if appMode == ModeWaves || appMode == ModeExport then
                case maybeWave of
                    Just wv ->
                        if showOverlayFill then
                            let eff = if isHov then Basics.min 1.0 (wv.opacity + 0.3) else wv.opacity
                            in "fill: " ++ waveColor wv.hue eff ++ ";"
                        else if isHov then "fill: rgba(64,120,255,0.2);"
                        else "fill: transparent;"
                    Nothing ->
                        if isHov then "fill: rgba(64,120,255,0.2);"
                        else if isSel then "fill: rgba(64,120,255,0.45);"
                        else "fill: transparent;"

            else
                if isHov then "fill: rgba(64,120,255,0.2);"
                else if isSel then "fill: rgba(64,120,255,0.45);"
                else "fill: transparent;"

        clsStr =
            [ "piece-overlay"
            , if isSel && maybeWave == Nothing then "selected" else ""
            ]
                |> List.filter ((/=) "")
                |> String.join " "

        clickMsg =
            if inGroupAssign then
                AssignPieceToGroup piece.id

            else if inWaveAssign then
                case ( maybeGroup, selectedWaveId ) of
                    ( Just g, Just wid ) -> AssignGroupToWave g.id wid
                    _ -> AssignPieceToWave piece.id

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

            pointerStyle =
                if isLassoing then "pointer-events: none; " else ""
        in
        Svg.polygon
            ([ SA.points pointsAttr
             , SA.class clsStr
             , SA.style (pointerStyle ++ fillStyle)
             ]
                ++ (if isLassoing then
                        []

                    else
                        [ onClick clickMsg
                        , onMouseEnter (SetHoveredPiece (Just piece.id))
                        , onMouseLeave (SetHoveredPiece Nothing)
                        ]
                   )
            )
            []


viewGrid : Float -> Float -> String -> Float -> List (Svg.Svg Msg)
viewGrid cw ch color houseUnitsHigh =
    let
        gridStep =
            ch / houseUnitsHigh

        -- Extend 1 unit beyond each side
        numV =
            floor (cw / gridStep) + 1

        numH =
            floor (ch / gridStep) + 1

        vLines =
            List.map
                (\i ->
                    let
                        x =
                            toFloat i * gridStep
                    in
                    Svg.line
                        [ SA.x1 (String.fromFloat x)
                        , SA.y1 (String.fromFloat -gridStep)
                        , SA.x2 (String.fromFloat x)
                        , SA.y2 (String.fromFloat (ch + gridStep))
                        , SA.stroke color
                        , SA.strokeWidth "1"
                        , attribute "vector-effect" "non-scaling-stroke"
                        ]
                        []
                )
                (List.range -1 numV)

        hLines =
            List.map
                (\i ->
                    let
                        y =
                            ch - toFloat i * gridStep
                    in
                    Svg.line
                        [ SA.x1 (String.fromFloat -gridStep)
                        , SA.y1 (String.fromFloat y)
                        , SA.x2 (String.fromFloat (cw + gridStep))
                        , SA.y2 (String.fromFloat y)
                        , SA.stroke color
                        , SA.strokeWidth "1"
                        , attribute "vector-effect" "non-scaling-stroke"
                        ]
                        []
                )
                (List.range -1 numH)
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


iconLockClosed : Html msg
iconLockClosed =
    Svg.svg [ SA.viewBox "0 0 24 24", SA.width "14", SA.height "14", SA.fill "currentColor" ]
        [ Svg.path [ SA.d "M6 22q-.825 0-1.412-.587T4 20V10q0-.825.588-1.412T6 8h1V6q0-2.075 1.463-3.537T12 1t3.538 1.463T17 6v2h1q.825 0 1.413.588T20 10v10q0 .825-.587 1.413T18 22zm0-2h12V10H6zm7.413-3.588Q14 15.826 14 15t-.587-1.412T12 13t-1.412.588T10 15t.588 1.413T12 17t1.413-.587M9 8h6V6q0-1.25-.875-2.125T12 3t-2.125.875T9 6zM6 20V10z" ] []
        ]


iconLockOpen : Html msg
iconLockOpen =
    Svg.svg [ SA.viewBox "0 0 24 24", SA.width "14", SA.height "14", SA.fill "currentColor" ]
        [ Svg.path [ SA.d "M6 20h12V10H6zm7.413-3.588Q14 15.826 14 15t-.587-1.412T12 13t-1.412.588T10 15t.588 1.413T12 17t1.413-.587M6 20V10zm0 2q-.825 0-1.412-.587T4 20V10q0-.825.588-1.412T6 8h7V6q0-2.075 1.463-3.537T18 1t3.538 1.463T23 6h-2q0-1.25-.875-2.125T18 3t-2.125.875T15 6v2h3q.825 0 1.413.588T20 10v10q0 .825-.587 1.413T18 22z" ] []
        ]


viewWaveRow : Model -> List Wave -> Wave -> Html Msg
viewWaveRow model allWaves wave =
    let
        isSelected =
            model.selectedWaveId == Just wave.id

        waveCount =
            List.length allWaves

        swatchColor =
            waveColor wave.hue 0.85
    in
    div
        [ classList
            [ ( "wave-row", True )
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
                [ if wave.locked then iconLockClosed else iconLockOpen ]
            , span
                [ class "wave-swatch"
                , style "background-color" swatchColor
                , stopPropagationOn "mousedown"
                    (D.map2 (\mx my -> ( StartColorPick (WaveColorTarget wave.id) mx my, True ))
                        (D.field "clientX" D.float)
                        (D.field "clientY" D.float)
                    )
                , title "Pick color"
                ]
                []
            , span [ class "wave-piece-count-label" ]
                [ text (String.fromInt (List.length wave.pieceIds) ++ " pcs") ]
            , span [ class "wave-row-spacer" ] []
            , span [ class "wave-actions" ]
                [ button
                    [ stopPropagationOn "click" (D.succeed ( RemoveWave wave.id, True ))
                    , disabled (waveCount <= 1)
                    , title "Delete wave"
                    ]
                    [ text "\u{2715}" ]
                ]
            ]
        , div [ class "wave-pieces" ]
            (toPieceDisplays model.groups wave.pieceIds
                |> List.indexedMap (\i display -> ( i + 1, display ))
                |> List.filterMap
                    (\( pos, display ) ->
                        case display of
                            SinglePiece pid ->
                                model.pieces |> List.filter (\p -> p.id == pid) |> List.head
                                    |> Maybe.map (\piece -> viewPieceThumb (Just ( wave.id, pid )) wave.locked model.hoveredPieceId pid (cacheBust piece.imgUrl model.pieceGeneration) (Just pos))

                            GroupedPiece repId allIds ->
                                model.pieces |> List.filter (\p -> p.id == repId) |> List.head
                                    |> Maybe.map (\piece -> viewGroupThumb (Just wave.id) model.hoveredPieceId (model.groups |> List.filter (\g -> List.member repId g.pieceIds) |> List.head) piece allIds model.pieceGeneration (Just pos) wave.locked)
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
                (toPieceDisplays model.groups (List.map .id unassignedPieces)
                    |> List.filterMap
                        (\display ->
                            case display of
                                SinglePiece pid ->
                                    model.pieces |> List.filter (\p -> p.id == pid) |> List.head
                                        |> Maybe.map (\p -> viewPieceThumb Nothing False model.hoveredPieceId p.id (p.imgUrl ++ "?v=" ++ String.fromInt model.pieceGeneration) Nothing)

                                GroupedPiece repId allIds ->
                                    model.pieces |> List.filter (\p -> p.id == repId) |> List.head
                                        |> Maybe.map (\p -> viewGroupThumb model.selectedWaveId model.hoveredPieceId (model.groups |> List.filter (\g -> List.member repId g.pieceIds) |> List.head) p allIds model.pieceGeneration Nothing False)
                        )
                )
            ]


viewPieceThumb : Maybe ( Int, String ) -> Bool -> Maybe String -> String -> String -> Maybe Int -> Html Msg
viewPieceThumb removeInfo isLocked hoveredId pieceId dataUrl maybePos =
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
         ]
            ++ (case maybePos of
                    Just pos ->
                        [ div [ class "tray-thumb-num" ] [ text (String.fromInt pos) ] ]

                    Nothing ->
                        []
               )
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


-- A thumbnail for a group of interchangeable pieces: shows one representative
-- image with an "xN" badge at the bottom. Clicking assigns/removes the whole group to/from
-- the wave identified by maybeWaveId. Draggable when not locked.
viewGroupThumb : Maybe Int -> Maybe String -> Maybe Group -> Piece -> List String -> Int -> Maybe Int -> Bool -> Html Msg
viewGroupThumb maybeWaveId hoveredId maybeGroup piece allIds generation maybePos isLocked =
    let
        n =
            List.length allIds

        isHovered =
            hoveredId == Just piece.id

        clickMsg =
            case ( maybeGroup, maybeWaveId ) of
                ( Just g, Just wid ) -> AssignGroupToWave g.id wid
                _ -> NoOp

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
        ([ classList [ ( "piece-thumb", True ), ( "hovered", isHovered ) ]
         , onMouseEnter (SetHoveredPiece (Just piece.id))
         , onMouseLeave (SetHoveredPiece Nothing)
         , onClick clickMsg
         ] ++ dragAttrs
        )
        [ img
            [ src (cacheBust piece.imgUrl generation)
            , style "max-height" "48px"
            , style "max-width" "80px"
            , style "display" "block"
            ]
            []
        , case maybePos of
            Just pos ->
                div [ class "piece-thumb-pos" ] [ text (String.fromInt pos) ]
            Nothing ->
                text ""
        , if n > 1 then
            div [ class "group-xn-badge group-xn-badge-bottom" ] [ text ("x" ++ String.fromInt n) ]
          else
            text ""
        ]


viewEditControls : Model -> List (Html Msg)
viewEditControls model =
    let
        changed =
            editHasChanges model

        pieceLabel =
            case model.selectedPieceId of
                Just pid ->
                    "Piece #" ++ pid

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
        [ text "Click a brick in the piece to remove it."
        , br [] []
        , text "Click a brick in another piece to merge it."
        , br [] []
        , text (String.fromInt brickCount ++ " brick" ++ (if brickCount == 1 then "" else "s") ++ " in piece.")
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

        Loading ->
            span [ class "status loading" ] [ text "Parsing PDF\u{2026}" ]

        Loaded _ ->
            text ""

        LoadError err ->
            span [ class "status error" ] [ text ("Error: " ++ err) ]


viewImportStats : LoadResponse -> Html Msg
viewImportStats response =
    let
        totalBricks =
            List.length response.bricks

        skipped =
            List.filter (String.startsWith "SKIPPED:") response.warnings |> List.length

        covered =
            List.filter (String.startsWith "COVERED:") response.warnings |> List.length

        -- Show MULTI_OBJECT warnings (layers with separate bricks)
        realWarnings =
            response.warnings
                |> List.filter (String.startsWith "MULTI_OBJECT:")
                |> List.map (String.replace "MULTI_OBJECT: " "")
    in
    div [ class "stats" ]
        ([ div [ class "row" ] [ text "Bricks imported", span [ class "val" ] [ text (String.fromInt totalBricks) ] ]
         ]
            ++ (if skipped > 0 then
                    [ div [ class "row" ] [ text "Skipped (no polygon)", span [ class "val" ] [ text (String.fromInt skipped) ] ] ]
                else
                    []
               )
            ++ (if covered > 0 then
                    [ div [ class "row" ] [ text "Covered (hidden)", span [ class "val" ] [ text (String.fromInt covered) ] ] ]
                else
                    []
               )
            ++ (if not (List.isEmpty realWarnings) then
                    [ div [ class "row", style "margin-top" "4px" ] [ text "Warnings:" ] ]
                        ++ List.map (\w -> div [ class "row", style "color" "#b04020", style "font-size" "10px" ] [ text w ]) realWarnings
                else
                    []
               )
        )


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


keyDecoder : D.Decoder Msg
keyDecoder =
    D.map3
        (\key ctrl shift ->
            if ctrl && key == "z" && not shift then
                Undo

            else if ctrl && key == "z" && shift then
                Redo

            else
                NoOp
        )
        (D.field "key" D.string)
        (D.field "ctrlKey" D.bool)
        (D.field "shiftKey" D.bool)


subscriptions : Model -> Sub Msg
subscriptions model =
    Sub.batch
        ([ Browser.Events.onKeyDown keyDecoder
         , tauriResponse TauriResponse
         ]
            ++ (case model.colorPicking of
                    Just _ ->
                        [ Browser.Events.onMouseMove
                            (D.map2 ColorPickMove
                                (D.field "clientX" D.float)
                                (D.field "clientY" D.float)
                            )
                        , Browser.Events.onMouseUp (D.succeed EndColorPick)
                        ]

                    Nothing ->
                        []
               )
        )



-- ── Main ─────────────────────────────────────────────────────────────────────


main : Program { version : String, isTauri : Bool } Model Msg
main =
    Browser.element
        { init = init
        , update = update
        , view = view
        , subscriptions = subscriptions
        }
