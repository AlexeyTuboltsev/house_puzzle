module Main exposing (main)

import Browser
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



-- ── Model ───────────────────────────────────────────────────────────────────


type LoadState
    = Idle
    | Loading
    | Loaded LoadResponse
    | LoadError String


type alias Model =
    { tifPath : String
    , loadState : LoadState

    -- Piece / wave state will be added in subsequent steps
    }


init : () -> ( Model, Cmd Msg )
init _ =
    ( { tifPath = ""
      , loadState = Idle
      }
    , Cmd.none
    )



-- ── Msg ─────────────────────────────────────────────────────────────────────


type Msg
    = SetTifPath String
    | RequestLoad
    | GotLoadResponse (Result Http.Error LoadResponse)



-- ── Update ──────────────────────────────────────────────────────────────────


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        SetTifPath path ->
            ( { model | tifPath = path }, Cmd.none )

        RequestLoad ->
            if String.isEmpty model.tifPath then
                ( model, Cmd.none )

            else
                ( { model | loadState = Loading }
                , loadTif model.tifPath
                )

        GotLoadResponse (Ok response) ->
            ( { model | loadState = Loaded response }, Cmd.none )

        GotLoadResponse (Err err) ->
            ( { model | loadState = LoadError (httpErrorToString err) }, Cmd.none )



-- ── HTTP ────────────────────────────────────────────────────────────────────


loadTif : String -> Cmd Msg
loadTif path =
    Http.post
        { url = "/api/load_tif"
        , body = Http.jsonBody (E.object [ ( "path", E.string path ) ])
        , expect = Http.expectJson GotLoadResponse decodeLoadResponse
        }


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

        Http.BadBody msg ->
            "Bad response: " ++ msg



-- ── View ─────────────────────────────────────────────────────────────────────


view : Model -> Html Msg
view model =
    div [ class "elm-app" ]
        [ viewHeader model
        , viewBody model
        ]


viewHeader : Model -> Html Msg
viewHeader model =
    header [ class "elm-header" ]
        [ h1 [] [ text "House Puzzle Editor" ]
        , div [ class "elm-load-controls" ]
            [ input
                [ type_ "text"
                , placeholder "e.g. in/casablanca 6.tif"
                , value model.tifPath
                , onInput SetTifPath
                , class "elm-path-input"
                ]
                []
            , button
                [ onClick RequestLoad
                , disabled (model.loadState == Loading || String.isEmpty model.tifPath)
                , class "elm-load-btn"
                ]
                [ text
                    (if model.loadState == Loading then
                        "Loading…"

                     else
                        "Load TIF"
                    )
                ]
            ]
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
            span [ class "elm-status ok" ]
                [ text
                    (String.fromInt (List.length r.bricks)
                        ++ " bricks loaded ("
                        ++ String.fromFloat r.canvas.width
                        ++ "×"
                        ++ String.fromFloat r.canvas.height
                        ++ " px canvas)"
                    )
                ]

        LoadError err ->
            span [ class "elm-status error" ] [ text ("Error: " ++ err) ]


viewBody : Model -> Html Msg
viewBody model =
    case model.loadState of
        Loaded response ->
            div [ class "elm-canvas-area" ]
                [ viewBrickSvg response ]

        _ ->
            div [ class "elm-placeholder" ]
                [ text "Load a TIF file to begin." ]


viewBrickSvg : LoadResponse -> Html Msg
viewBrickSvg response =
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
        (List.map viewBrickPath response.bricks)


viewBrickPath : Brick -> Svg.Svg Msg
viewBrickPath brick =
    let
        -- polygon points are brick-local; offset to canvas coords
        absPoints =
            List.map (\( x, y ) -> ( x + brick.x, y + brick.y )) brick.polygon

        pointsAttr =
            absPoints
                |> List.map (\( x, y ) -> String.fromFloat x ++ "," ++ String.fromFloat y)
                |> String.join " "
    in
    if List.isEmpty absPoints then
        -- fallback: rectangle for bricks without a polygon
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



-- ── Main ─────────────────────────────────────────────────────────────────────


main : Program () Model Msg
main =
    Browser.element
        { init = init
        , update = update
        , view = view
        , subscriptions = \_ -> Sub.none
        }
