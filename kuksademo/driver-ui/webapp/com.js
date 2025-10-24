var SUB_ID_SPEED=0;
var SUB_ID_FL=0;
var SUB_ID_FR=0;
var SUB_ID_RL=0;
var SUB_ID_RR=0;


var wscon = null;


function setSpeed(speed) {
    $('#acfcanG').attr('data-value', speed);
    //console.log("New Speed is "+speed);
}


function initAll() {
    initWebsocket();
}

function statusMessage(msg) {
    $("#status").html(msg);
}

function keepAlive( ) {
	aliveMSG = { action: "get", path: "Vehicle.VehicleIdentification.VIN", "requestId": "99" }
	if (wscon != null && wscon.readyState == WebSocket.OPEN) {
		wscon.send(JSON.stringify(aliveMSG));
	}
	setTimeout(keepAlive,2000); //we need to regularly send data thorugh ws to detect disconnects


}

function initWebsocket() {
    host=window.location.hostname
    wscon = new WebSocket("ws://"+host+":8090");

    statusMessage("Opening  websocket...")
    wscon.onopen = function () {
        console.log("Open. Send Auhtorize");
        statusMessage("Websocket open. Sending authorization & subsription request...")
        //authMsg = { action: "authorize", tokens: TOKEN, requestId: "1" }
        //wscon.send(JSON.stringify(authMsg));

        subMsg = { action: "subscribe", path: "Vehicle.Speed", "requestId": "2" }
        wscon.send(JSON.stringify(subMsg));

        subMsg = { action: "subscribe", path: "Vehicle.Chassis.Axle.Row1.Wheel.Left.Brake.PadWear", "requestId": "10" }
        wscon.send(JSON.stringify(subMsg));
        subMsg = { action: "subscribe", path: "Vehicle.Chassis.Axle.Row1.Wheel.Right.Brake.PadWear", "requestId": "11" }
        wscon.send(JSON.stringify(subMsg));
        subMsg = { action: "subscribe", path: "Vehicle.Chassis.Axle.Row2.Wheel.Left.Brake.PadWear", "requestId": "12" }
        wscon.send(JSON.stringify(subMsg));
        subMsg = { action: "subscribe", path: "Vehicle.Chassis.Axle.Row2.Wheel.Right.Brake.PadWear", "requestId": "13" }
        wscon.send(JSON.stringify(subMsg));

        setTimeout(keepAlive,2000); //we need to regularly send data thorugh ws to detect disconnects

    };


    wscon.onerror = function () {
        //console.log("Websocket error, try reconnection");
        statusMessage("Websocket connection error. Reconnecting...")
        //onclose will be called anyway
        //setTimeout(initWebsocket,500);
    };

    wscon.onclose = function () {
        console.log("Websocket was closed, try reconnection");
        statusMessage("Websocket unexpectedly closed. Reconnecting...")
        setTimeout(initWebsocket,500);
    };

    wscon.onmessage = function (e) {
        jsonobj = JSON.parse(e.data);
        if ( jsonobj.hasOwnProperty("requestId") ) {
            if (jsonobj['requestId'] == 2) {
                SUB_ID_SPEED=jsonobj['subscriptionId'];
                statusMessage("Speed subcription succeeded.")
            }
            else if (jsonobj['requestId'] == 10) {
                SUB_ID_FL=jsonobj['subscriptionId'];
                statusMessage("FL subcription succeeded.")
            }
            else if (jsonobj['requestId'] == 11) {
                SUB_ID_FR=jsonobj['subscriptionId'];
                statusMessage("FR subcription succeeded.")
            }
            else if (jsonobj['requestId'] == 12) {
                SUB_ID_RL=jsonobj['subscriptionId'];
                statusMessage("RL subcription succeeded.")
            }
            else if (jsonobj['requestId'] == 13) {
                SUB_ID_RR=jsonobj['subscriptionId'];
                statusMessage("RR subcription succeeded.")
            }
            else if (jsonobj['requestId'] == 99) {
                    return
			}

        }
        if ( jsonobj.hasOwnProperty("action") ) {
            if (jsonobj['action'] == "subscription" && jsonobj.hasOwnProperty("data") ) {
                parseData(jsonobj);
                statusMessage("Receiving data")
                return;
            }
        }
        console.log("Received control message "+e.data); // Send the message 'Ping' to the server
    };

}

function tempTostr(wear) {
    console.log("Wear is "+wear);
    if (wear < 50) {
        return "🟢 "+wear
    }
    else if (wear < 85) {
        return "🟡 "+wear
    }
    else if (wear < 99) {
        return "🟠 "+wear
        return "🛠 "+wear
    }
    return "🔴 🛠 ⚠ "+wear
}
function parseData(js) {
    if ( js.hasOwnProperty("subscriptionId") ) {
        if (js['subscriptionId'] == SUB_ID_SPEED) {
            setSpeed(js['data']['dp']['value']);
            return;
        }
        else if (js['subscriptionId'] == SUB_ID_FL) {
            $('.front-left').html(tempTostr(js['data']['dp']['value']));
            return;
        }
        else if (js['subscriptionId'] == SUB_ID_FR) {
            $('.front-right').html(tempTostr(js['data']['dp']['value']));
            return;
        }
        else if (js['subscriptionId'] == SUB_ID_RL) {
            $('.rear-left').html(tempTostr(js['data']['dp']['value']));
            return;
        }
        else if (js['subscriptionId'] == SUB_ID_RR) {
            $('.rear-right').html(tempTostr(js['data']['dp']['value']));
            return;
        }
    }
    console.log("Received unknown data "+JSON.stringify(js));
}
