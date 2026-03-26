port module Main exposing (main)

import Browser
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
    , pieceIds : List Int
    }


type ViewMode
    = ViewPieces
    | ViewBlueprint



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
    , viewMode : ViewMode
    , showOutlines : Bool
    , showGrid : Bool
    , waves : List Wave
    , nextWaveId : Int
    , selectedPieceId : Maybe Int
    }


init : () -> ( Model, Cmd Msg )
init _ =
    ( { selectedFileName = ""
      , loadState = Idle
      , targetCount = 10
      , minBorder = 5
      , seed = 42
      , generateState = NotGenerated
      , pieces = []
      , pieceImages = Dict.empty
      , bricksById = Dict.empty
      , viewMode = ViewPieces
      , showOutlines = True
      , showGrid = False
      , waves = []
      , nextWaveId = 1
      , selectedPieceId = Nothing
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
    | SetViewMode ViewMode
    | ToggleOutlines Bool
    | ToggleGrid Bool
    | AddWave
    | ToggleWaveVisibility Int
    | SelectPiece Int



-- ── Ports ───────────────────────────────────────────────────────────────────


port compositePieces : E.Value -> Cmd msg


port gotPieceImages : (E.Value -> msg) -> Sub msg



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
                        | pieceImages = Dict.fromList images
                        , generateState = Generated
                      }
                    , Cmd.none
                    )

                Err _ ->
                    ( { model | generateState = NotGenerated }, Cmd.none )

        SetViewMode mode ->
            ( { model | viewMode = mode }, Cmd.none )

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
                    , pieceIds = []
                    }
            in
            ( { model
                | waves = model.waves ++ [ newWave ]
                , nextWaveId = model.nextWaveId + 1
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
        [ viewHeader model
        , div [ class "main" ]
            [ viewSidebar model
            , viewCanvasArea model
            , viewWavesPanel model
            ]
        ]


viewHeader : Model -> Html Msg
viewHeader model =
    div [ class "header" ]
        [ h1 [] [ text "House Puzzle Editor" ]
        , button
            [ class "primary"
            , disabled (model.generateState /= Generated)
            ]
            [ text "Export ZIP" ]
        ]


viewSidebar : Model -> Html Msg
viewSidebar model =
    let
        isLoaded =
            case model.loadState of
                Loaded _ ->
                    True

                _ ->
                    False

        isCompositing =
            model.generateState == Compositing

        busy =
            model.loadState == Uploading || model.loadState == Loading
    in
    div [ class "sidebar" ]
        ([ h2 [] [ text "Source TIF" ]
         , button
             [ onClick PickFile
             , disabled busy
             , style "width" "100%"
             , style "margin-bottom" "4px"
             ]
             [ text "Open TIF\u{2026}" ]
         ]
            ++ (if not (String.isEmpty model.selectedFileName) then
                    [ div
                        [ style "font-size" "11px"
                        , style "color" "#e0a050"
                        , style "margin-bottom" "6px"
                        , style "overflow" "hidden"
                        , style "text-overflow" "ellipsis"
                        , style "white-space" "nowrap"
                        ]
                        [ text model.selectedFileName ]
                    ]

                else
                    []
               )
            ++ [ viewStatusBadge model ]
            ++ (if isLoaded then
                    [ h2 [] [ text "View" ]
                    , div [ class "view-toggles" ]
                        [ button
                            [ classList [ ( "active", model.viewMode == ViewPieces ) ]
                            , onClick (SetViewMode ViewPieces)
                            ]
                            [ text "Pieces" ]
                        , button
                            [ classList [ ( "active", model.viewMode == ViewBlueprint ) ]
                            , onClick (SetViewMode ViewBlueprint)
                            ]
                            [ text "Blueprint" ]
                        ]
                    , div [ class "checkbox-group" ]
                        [ input
                            [ type_ "checkbox"
                            , id "showOutlines"
                            , checked model.showOutlines
                            , onCheck ToggleOutlines
                            ]
                            []
                        , label [ for "showOutlines" ] [ text "Show piece outlines" ]
                        ]
                    , div [ class "checkbox-group" ]
                        [ input
                            [ type_ "checkbox"
                            , id "showGrid"
                            , checked model.showGrid
                            , onCheck ToggleGrid
                            ]
                            []
                        , label [ for "showGrid" ] [ text "Show grid" ]
                        ]
                    , h2 [] [ text "Puzzle Parameters" ]
                    , div [ class "param-group" ]
                        [ label []
                            [ text "Target Pieces "
                            , span [ class "value" ] [ text (String.fromInt model.targetCount) ]
                            ]
                        , input
                            [ type_ "range"
                            , Html.Attributes.min "5"
                            , Html.Attributes.max "181"
                            , value (String.fromInt model.targetCount)
                            , onInput SetTargetCount
                            ]
                            []
                        ]
                    , div [ class "param-group" ]
                        [ label []
                            [ text "Min Border "
                            , span [ class "value" ] [ text (String.fromInt model.minBorder) ]
                            , text "px"
                            ]
                        , input
                            [ type_ "range"
                            , Html.Attributes.min "0"
                            , Html.Attributes.max "50"
                            , value (String.fromInt model.minBorder)
                            , onInput SetMinBorder
                            ]
                            []
                        ]
                    , div [ class "param-group" ]
                        [ label []
                            [ text "Seed "
                            , span [ class "value" ] [ text (String.fromInt model.seed) ]
                            ]
                        , input
                            [ type_ "number"
                            , value (String.fromInt model.seed)
                            , onInput SetSeed
                            , Html.Attributes.min "0"
                            , Html.Attributes.max "99999"
                            ]
                            []
                        ]
                    , div [ class "btn-row" ]
                        [ button
                            [ class "primary"
                            , onClick RequestGenerate
                            , disabled isCompositing
                            ]
                            [ text
                                (if isCompositing then
                                    "Generating\u{2026}"

                                 else
                                    "Generate Puzzle"
                                )
                            ]
                        ]
                    , div [ class "btn-row" ]
                        [ button
                            [ disabled (model.selectedPieceId == Nothing) ]
                            [ text
                                (case model.selectedPieceId of
                                    Just pid ->
                                        "Edit Piece #" ++ String.fromInt pid

                                    Nothing ->
                                        "Edit Piece"
                                )
                            ]
                        ]
                    , h2 [] [ text "Stats" ]
                    , viewStats model
                    ]

                else
                    []
               )
            ++ [ div
                    [ style "margin-top" "auto"
                    , style "padding-top" "12px"
                    , style "font-size" "10px"
                    , style "color" "#555"
                    ]
                    [ text "Elm" ]
               ]
        )


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


viewCanvasArea : Model -> Html Msg
viewCanvasArea model =
    div [ class "canvas-area" ]
        [ case model.loadState of
            Loaded response ->
                viewMainSvg response model

            _ ->
                div [ class "canvas-info" ] [ text "Select a TIF to begin" ]
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
            model.viewMode == ViewPieces

        showPieceImages =
            isPieces && isGenerated && not (Dict.isEmpty model.pieceImages)

        showComposite =
            isPieces && not isGenerated && response.hasComposite

        -- Base layer
        baseLayer =
            if showPieceImages then
                List.map (viewPieceImage model.pieceImages) model.pieces

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
                -- Blueprint post-gen: merged piece polygons
                List.map viewPieceBlueprintPath model.pieces

            else
                -- Pre-gen: individual brick polygons
                List.map viewBrickPath response.bricks

        -- Composite brick hover overlays (pre-gen only)
        compositeOverlays =
            if showComposite then
                List.map viewBrickOverlay response.bricks

            else
                []

        -- Grid lines
        gridLayer =
            if model.showGrid then
                viewGrid cw ch model.viewMode

            else
                []

        -- Piece outlines — white rect borders around each piece (post-gen only)
        outlineLayer =
            if isGenerated && model.showOutlines then
                List.map viewPieceOutline model.pieces

            else
                []

        -- Piece interaction overlays (hover + click, post-gen only)
        pieceOverlays =
            if isGenerated then
                List.map (viewPieceOverlay model.selectedPieceId) model.pieces

            else
                []
    in
    Svg.svg
        [ SA.viewBox ("0 0 " ++ w ++ " " ++ h)
        , SA.class "house-svg"
        , SA.width w
        , SA.height h
        ]
        (baseLayer
            ++ compositeOverlays
            ++ gridLayer
            ++ outlineLayer
            ++ pieceOverlays
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
            ]
            []

    else
        Svg.polygon
            [ SA.points pointsAttr
            , SA.fill "transparent"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "brick-overlay"
            ]
            []


viewGrid : Float -> Float -> ViewMode -> List (Svg.Svg Msg)
viewGrid cw ch viewMode =
    let
        gridStep =
            211.0

        color =
            if viewMode == ViewBlueprint then
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
                            toFloat i * gridStep
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


-- Renders the merged piece shape as a blueprint polygon (blue fill, white stroke).
-- Piece polygon is in canvas coords (absolute), unlike brick polygons which are brick-local.
viewPieceBlueprintPath : Piece -> Svg.Svg Msg
viewPieceBlueprintPath piece =
    if List.isEmpty piece.polygon then
        Svg.rect
            [ SA.x (String.fromFloat piece.x)
            , SA.y (String.fromFloat piece.y)
            , SA.width (String.fromFloat piece.width)
            , SA.height (String.fromFloat piece.height)
            , SA.fill "#2a5da8"
            , SA.stroke "white"
            , SA.strokeWidth "4"
            , attribute "vector-effect" "non-scaling-stroke"
            , attribute "paint-order" "fill stroke"
            , SA.class "brick-path"
            ]
            []

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


-- Renders the piece outline (transparent fill, white stroke) on top of composite images.
viewPieceOutline : Piece -> Svg.Svg Msg
viewPieceOutline piece =
    if List.isEmpty piece.polygon then
        Svg.rect
            [ SA.x (String.fromFloat piece.x)
            , SA.y (String.fromFloat piece.y)
            , SA.width (String.fromFloat piece.width)
            , SA.height (String.fromFloat piece.height)
            , SA.fill "transparent"
            , SA.stroke "white"
            , SA.strokeWidth "2"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "piece-outline"
            ]
            []

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
            , SA.stroke "white"
            , SA.strokeWidth "2"
            , SA.strokeLinejoin "round"
            , attribute "vector-effect" "non-scaling-stroke"
            , SA.class "piece-outline"
            ]
            []


viewPieceOverlay : Maybe Int -> Piece -> Svg.Svg Msg
viewPieceOverlay selectedId piece =
    let
        isSelected =
            selectedId == Just piece.id

        fillColor =
            if isSelected then
                "rgba(64,120,255,0.35)"

            else
                "transparent"
    in
    Svg.rect
        [ SA.x (String.fromFloat piece.x)
        , SA.y (String.fromFloat piece.y)
        , SA.width (String.fromFloat piece.width)
        , SA.height (String.fromFloat piece.height)
        , SA.fill fillColor
        , SA.class
            (if isSelected then
                "piece-overlay selected"

             else
                "piece-overlay"
            )
        , onClick (SelectPiece piece.id)
        ]
        []


viewWavesPanel : Model -> Html Msg
viewWavesPanel model =
    let
        assignedIds =
            List.concatMap .pieceIds model.waves

        assignedCount =
            List.length assignedIds

        totalPieces =
            List.length model.pieces

        unassignedPieces =
            List.filter (\p -> not (List.member p.id assignedIds)) model.pieces
    in
    div [ class "waves-panel-wrapper" ]
        [ div [ class "waves-resize-handle" ] []
        , div [ class "waves-panel" ]
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
                [ button [ onClick AddWave ] [ text "+ Wave" ] ]
            , div [ class "waves-body" ]
                (List.map (viewWaveRow model) model.waves
                    ++ [ viewUnassignedRow model unassignedPieces ]
                )
            ]
        ]


viewWaveRow : Model -> Wave -> Html Msg
viewWaveRow model wave =
    div [ class "wave-row" ]
        [ div [ class "wave-row-header" ]
            [ span
                [ classList
                    [ ( "wave-eye", True )
                    , ( "hidden", not wave.visible )
                    ]
                , onClick (ToggleWaveVisibility wave.id)
                ]
                [ text "\u{1F441}" ]
            , span [ class "wave-label" ] [ text wave.name ]
            , span [ class "wave-piece-count" ]
                [ text (String.fromInt (List.length wave.pieceIds) ++ " pieces") ]
            ]
        , div [ class "wave-pieces" ]
            (List.filterMap
                (\pid ->
                    Dict.get pid model.pieceImages
                        |> Maybe.map (viewPieceThumb pid)
                )
                wave.pieceIds
            )
        ]


viewUnassignedRow : Model -> List Piece -> Html Msg
viewUnassignedRow model unassignedPieces =
    if List.isEmpty model.pieces then
        text ""

    else
        div [ class "wave-row" ]
            [ div [ class "wave-row-header" ]
                [ span [ class "wave-label unassigned-label" ] [ text "Unassigned" ]
                , span [ class "wave-piece-count" ]
                    [ text (String.fromInt (List.length unassignedPieces) ++ " pieces") ]
                ]
            , div [ class "wave-pieces" ]
                (List.filterMap
                    (\p ->
                        Dict.get p.id model.pieceImages
                            |> Maybe.map (viewPieceThumb p.id)
                    )
                    unassignedPieces
                )
            ]


viewPieceThumb : Int -> String -> Html Msg
viewPieceThumb pieceId dataUrl =
    div [ class "piece-thumb" ]
        [ img
            [ src dataUrl
            , style "max-height" "48px"
            , style "max-width" "80px"
            , style "display" "block"
            ]
            []
        , div [ class "piece-thumb-label" ] [ text ("#" ++ String.fromInt pieceId) ]
        ]



-- ── Subscriptions ────────────────────────────────────────────────────────────


subscriptions : Model -> Sub Msg
subscriptions _ =
    gotPieceImages GotPieceImages



-- ── Main ─────────────────────────────────────────────────────────────────────


main : Program () Model Msg
main =
    Browser.element
        { init = init
        , update = update
        , view = view
        , subscriptions = subscriptions
        }
