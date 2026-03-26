port module Main exposing (main)

import Browser
import Dict exposing (Dict)
import Html exposing (..)
import Html.Attributes exposing (..)
import Html.Events exposing (..)
import Http
import Json.Decode as D
import Json.Encode as E
import Svg
import Svg.Attributes as SA



-- ── Types ──────────────────────────────────────────────────────────────────


type alias TifEntry =
    { name : String
    , path : String
    , sizeMb : Float
    }


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
    -- lightweight brick info inside a piece (from /api/merge)
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


type alias Model =
    { tifPath : String
    , tifList : List TifEntry
    , loadState : LoadState
    , targetCount : Int
    , generateState : GenerateState
    , pieces : List Piece
    , pieceImages : Dict Int String  -- piece id → data URL
    , bricksById : Dict Int Brick
    }


init : () -> ( Model, Cmd Msg )
init _ =
    ( { tifPath = ""
      , tifList = []
      , loadState = Idle
      , targetCount = 10
      , generateState = NotGenerated
      , pieces = []
      , pieceImages = Dict.empty
      , bricksById = Dict.empty
      }
    , fetchTifList
    )



-- ── Msg ─────────────────────────────────────────────────────────────────────


type Msg
    = GotTifList (Result Http.Error (List TifEntry))
    | SetTifPath String
    | RequestLoad
    | GotLoadResponse (Result Http.Error LoadResponse)
    | SetTargetCount String
    | RequestGenerate
    | GotMergeResponse (Result Http.Error MergeResponse)
    | GotPieceImages E.Value



-- ── Ports ───────────────────────────────────────────────────────────────────


port compositePieces : E.Value -> Cmd msg


port gotPieceImages : (E.Value -> msg) -> Sub msg



-- ── Update ──────────────────────────────────────────────────────────────────


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        GotTifList (Ok entries) ->
            ( { model
                | tifList = entries
                , tifPath =
                    -- pre-select first entry if nothing chosen yet
                    if String.isEmpty model.tifPath then
                        entries |> List.head |> Maybe.map .path |> Maybe.withDefault ""

                    else
                        model.tifPath
              }
            , Cmd.none
            )

        GotTifList (Err _) ->
            ( model, Cmd.none )

        SetTifPath path ->
            ( { model | tifPath = path }, Cmd.none )

        RequestLoad ->
            if String.isEmpty model.tifPath then
                ( model, Cmd.none )

            else
                ( { model
                    | loadState = Loading
                    , generateState = NotGenerated
                    , pieces = []
                    , pieceImages = Dict.empty
                  }
                , loadTif model.tifPath
                )

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

        RequestGenerate ->
            case model.loadState of
                Loaded _ ->
                    ( { model
                        | generateState = Compositing
                        , pieces = []
                        , pieceImages = Dict.empty
                      }
                    , mergeBricks model.targetCount
                    )

                _ ->
                    ( model, Cmd.none )

        GotMergeResponse (Ok response) ->
            -- Fire the compositing port; JS will call back with images
            ( { model | pieces = response.pieces }
            , compositePieces (encodePieceList response.pieces)
            )

        GotMergeResponse (Err _) ->
            ( { model | generateState = NotGenerated }
            , Cmd.none
            )

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



-- ── HTTP ────────────────────────────────────────────────────────────────────


fetchTifList : Cmd Msg
fetchTifList =
    Http.get
        { url = "/api/list_tifs"
        , expect = Http.expectJson GotTifList decodeTifList
        }


decodeTifList : D.Decoder (List TifEntry)
decodeTifList =
    D.field "tifs"
        (D.list
            (D.map3 TifEntry
                (D.field "name" D.string)
                (D.field "path" D.string)
                (D.field "size_mb" D.float)
            )
        )


loadTif : String -> Cmd Msg
loadTif path =
    Http.post
        { url = "/api/load_tif"
        , body = Http.jsonBody (E.object [ ( "path", E.string path ) ])
        , expect = Http.expectJson GotLoadResponse decodeLoadResponse
        }


mergeBricks : Int -> Cmd Msg
mergeBricks targetCount =
    Http.post
        { url = "/api/merge"
        , body =
            Http.jsonBody
                (E.object
                    [ ( "target_count", E.int targetCount )
                    , ( "seed", E.int 42 )
                    , ( "min_border", E.int 5 )
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
    D.map7 Piece
        (D.field "id" D.int)
        (D.field "x" D.float)
        (D.field "y" D.float)
        (D.field "width" D.float)
        (D.field "height" D.float)
        (D.field "brick_ids" (D.list D.int))
        (D.field "bricks" (D.list decodeBrickRef))


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
    div [ class "elm-app" ]
        [ viewHeader model
        , viewBody model
        ]


viewHeader : Model -> Html Msg
viewHeader model =
    let
        isLoaded =
            case model.loadState of
                Loaded _ ->
                    True

                _ ->
                    False

        isLoading =
            model.loadState == Loading

        isCompositing =
            model.generateState == Compositing
    in
    header [ class "elm-header" ]
        [ h1 [] [ text "House Puzzle Editor" ]
        , div [ class "elm-load-controls" ]
            [ if List.isEmpty model.tifList then
                input
                    [ type_ "text"
                    , placeholder "e.g. in/casablanca 6.tif"
                    , value model.tifPath
                    , onInput SetTifPath
                    , class "elm-path-input"
                    ]
                    []

              else
                select
                    [ onInput SetTifPath
                    , class "elm-path-input"
                    ]
                    (List.map
                        (\t ->
                            option
                                [ value t.path
                                , selected (t.path == model.tifPath)
                                ]
                                [ text (t.name ++ " (" ++ String.fromFloat t.sizeMb ++ " MB)") ]
                        )
                        model.tifList
                    )
            , button
                [ onClick RequestLoad
                , disabled (isLoading || String.isEmpty model.tifPath)
                , class "elm-load-btn"
                ]
                [ text
                    (if isLoading then
                        "Loading…"

                     else
                        "Load TIF"
                    )
                ]
            ]
        , if isLoaded then
            div [ class "elm-load-controls" ]
                [ label [ class "elm-label" ] [ text "Pieces:" ]
                , input
                    [ type_ "number"
                    , value (String.fromInt model.targetCount)
                    , onInput SetTargetCount
                    , Html.Attributes.min "1"
                    , Html.Attributes.max "100"
                    , class "elm-count-input"
                    ]
                    []
                , button
                    [ onClick RequestGenerate
                    , disabled isCompositing
                    , class "elm-load-btn"
                    ]
                    [ text
                        (if isCompositing then
                            "Generating…"

                         else
                            "Generate Puzzle"
                        )
                    ]
                ]

          else
            text ""
        , viewStatus model
        ]


viewStatus : Model -> Html Msg
viewStatus model =
    case model.loadState of
        Idle ->
            text ""

        Loading ->
            span [ class "elm-status loading" ] [ text "Parsing TIF and tracing brick outlines…" ]

        Loaded r ->
            let
                suffix =
                    case model.generateState of
                        Generated ->
                            " — " ++ String.fromInt (List.length model.pieces) ++ " pieces"

                        Compositing ->
                            " — compositing pieces…"

                        NotGenerated ->
                            ""
            in
            span [ class "elm-status ok" ]
                [ text
                    (String.fromInt (List.length r.bricks)
                        ++ " bricks ("
                        ++ String.fromFloat r.canvas.width
                        ++ "×"
                        ++ String.fromFloat r.canvas.height
                        ++ ")"
                        ++ suffix
                    )
                ]

        LoadError err ->
            span [ class "elm-status error" ] [ text ("Error: " ++ err) ]


viewBody : Model -> Html Msg
viewBody model =
    case model.loadState of
        Loaded response ->
            div [ class "elm-canvas-area" ]
                [ viewMainSvg response model ]

        _ ->
            div [ class "elm-placeholder" ]
                [ text "Load a TIF file to begin." ]


viewMainSvg : LoadResponse -> Model -> Html Msg
viewMainSvg response model =
    let
        w =
            String.fromFloat response.canvas.width

        h =
            String.fromFloat response.canvas.height
    in
    Svg.svg
        [ SA.viewBox ("0 0 " ++ w ++ " " ++ h)
        , SA.class "elm-brick-svg"
        , SA.width w
        , SA.height h
        ]
        (if model.generateState == Generated && not (Dict.isEmpty model.pieceImages) then
            List.map (viewPieceImage model.pieceImages) model.pieces

         else
            List.map viewBrickPath response.bricks
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
                , SA.class "elm-piece-image"
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
            , SA.fill "none"
            , SA.stroke "#4af"
            , SA.strokeWidth "1"
            , SA.opacity "0.4"
            ]
            []

    else
        Svg.polygon
            [ SA.points pointsAttr
            , SA.fill "rgba(64,170,255,0.08)"
            , SA.stroke "#4af"
            , SA.strokeWidth "1"
            , SA.strokeLinejoin "round"
            , attribute "data-brick-id" (String.fromInt brick.id)
            , SA.class "elm-brick-path"
            ]
            []



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
