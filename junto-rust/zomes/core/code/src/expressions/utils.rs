//Holochain core imports
use hdk::{
    error::{ZomeApiResult, ZomeApiError},
    holochain_core_types::{
        cas::content::Address, 
        entry::Entry, 
        entry::AppEntryValue,
        hash::HashString,
        link::LinkMatch
    },
    holochain_wasm_utils::api_serialization::{
        get_entry::{
            GetEntryOptions, GetEntryResultType
        }
    }
};

use std::convert::TryFrom;
use std::collections::HashSet;
use std::collections::HashMap;
use std::hash::Hash;

//Our module(s) imports
use super::group;
use super::collection;
use super::time;
use super::indexing;
use super::definitions::{
    app_definitions,
    function_definitions::{
        FunctionDescriptor,
        FunctionParameters,
        EntryAndAddressResult,
        EntryAndAddress,
        HooksResultTypes,
        ExpressionData,
        ContextAuthResult
    }
};

//This is a helper function which allows us to easily and dynamically handle all functions calls that need to happen
pub fn handle_hooks(hooks: Vec<FunctionDescriptor>) -> ZomeApiResult<Vec<HooksResultTypes>> {
    //First we get all hook functions which can be run on given expression types
    let mut hook_result_outputs = vec![];
    for hook_descriptor in hooks{ //iterate over hook function names provided in function call
        match hook_descriptor.name{ //Match function names
            "time_to_expression" => {
                match &hook_descriptor.parameters{
                    FunctionParameters::TimeToExpression {link_type, tag, direction, expression_address} => {
                        hdk::debug("Running time_to_expression")?;
                        let time_addresses = time::time_to_expression(link_type, tag, direction, &expression_address)?;
                        hdk::debug("Ran time_to_expression")?;
                        hook_result_outputs.push(HooksResultTypes::TimeToExpression(time_addresses));
                    },
                    _ => return Err(ZomeApiError::from("time_to_expresssion expects the LocalTimeToExpression enum value to be present".to_string()))
                }
            },
            "create_pack" => {
                match &hook_descriptor.parameters{
                    FunctionParameters::CreatePack {username_address, first_name} =>{
                        hdk::debug("Running create_pack")?;
                        let pack = group::create_pack(username_address, first_name.to_string())?;
                        hdk::debug(format!("Ran create_pack, pack address is: {:?}", pack.clone()))?;
                        hook_result_outputs.push(HooksResultTypes::CreatePack(pack))
                    },
                    _ => return Err(ZomeApiError::from("create_pack expectes the CreatePack enum value to be present".to_string()))
                }
            },
            "create_den" => {
                match &hook_descriptor.parameters{
                    FunctionParameters::CreateDen {username_address, first_name} =>{
                        hdk::debug("Running create_den")?;
                        let dens = collection::create_den(username_address, first_name.to_string())?;
                        hdk::debug(format!("Ran create_den, dens: {:?}", dens.clone()))?;
                        hook_result_outputs.push(HooksResultTypes::CreateDen(dens))
                    },
                    _ => return Err(ZomeApiError::from("create_den expectes the CreateDen enum value to be present".to_string()))
                }
            },
            "link_expression" => {
                match &hook_descriptor.parameters{
                    FunctionParameters::LinkExpression {link_type, tag, direction, parent_expression, child_expression} =>{
                        hdk::debug("Running link_expression")?;
                        let link_result = link_expression(link_type, tag, direction, &parent_expression, &child_expression)?;
                        hdk::debug("Ran link_expression")?;
                        hook_result_outputs.push(HooksResultTypes::LinkExpression(link_result))
                    },
                    _ => return Err(ZomeApiError::from("link_expression expects the LinkExpression enum value to be present".to_string()))
                }
            },
            "create_post_index" => {
                match &hook_descriptor.parameters{
                    FunctionParameters::CreatePostIndex {indexes, context, expression, index_string, link_type} =>{
                        hdk::debug("Running create_post_index")?;
                        let query_point_result = indexing::create_post_index(indexes, context, expression, index_string, link_type)?;
                        hdk::debug("Ran create_post_index")?;
                        hook_result_outputs.push(HooksResultTypes::CreatePostIndex(query_point_result))
                    },
                    _ => return Err(ZomeApiError::from("create_post_index expects the CreatePostIndex enum value to be present".to_string()))
                }
            },
            &_ => {
                return Err(ZomeApiError::from("Specified function does not exist".to_string()))
            }
        };
    };
    Ok(hook_result_outputs) //success
}

//Link two expression objects together in a given direction
pub fn link_expression(link_type: &str, tag: &str, direction: &str, parent_expression: &Address, child_expression: &Address) -> ZomeApiResult<&'static str>{
    hdk::debug("Linking expressions")?;
    if (direction == "reverse") | (direction == "both"){
        hdk::debug(format!("Linking expression: {} (child) to: {} (parent) with tag: {} and link_type: {}", child_expression, parent_expression, tag, link_type))?;
        hdk::link_entries(child_expression, parent_expression, link_type, tag)?;
    }
    if (direction == "forward") | (direction == "both"){
        hdk::debug(format!("Linking expression: {} (parent) to: {} (child) with tag: {} and link_type: {}", parent_expression, child_expression, tag, link_type))?;
        hdk::link_entries(parent_expression, child_expression, link_type, tag)?;
    }
    Ok("Links between expressions made with specified tag")
}

pub fn get_links_and_load(
    base: &HashString,
    link_type: LinkMatch<&str>,
    tag: LinkMatch<&str>
) -> ZomeApiResult<EntryAndAddressResult<Entry>>  {
	let get_links_result = hdk::get_links(base, link_type, tag)?;

	Ok(get_links_result.addresses()
	.iter()
	.map(|address| {
		hdk::get_entry(&address.to_owned())
		.map(|entry: Option<Entry>| {
			EntryAndAddress{
				address: address.to_owned(),
				entry: entry.unwrap()
			}
		})
	})
	.filter_map(Result::ok)
	.collect())
}

//This function has now been implemented in the HDK - but its still useful as it can return the address as well as the entry
pub fn get_links_and_load_type<R: TryFrom<AppEntryValue>>(base: &HashString, link_type: LinkMatch<&str>, tag: LinkMatch<&str>) -> ZomeApiResult<EntryAndAddressResult<R>> {
	let link_load_results = get_links_and_load(base, link_type, tag)?;

	Ok(link_load_results
	.iter()
	.map(|get_links_result| {

		match get_links_result.entry.clone() {
			Entry::App(_, entry_value) => {
				let entry = R::try_from(entry_value)
				.map_err(|_| ZomeApiError::Internal(
					"Could not convert get_links result to requested type".to_string())
				)?;

	            Ok(EntryAndAddress::<R>{
	                entry: entry, 
	                address: get_links_result.address.clone()
	            })
			},
			_ => Err(ZomeApiError::Internal(
				"get_links did not return an app entry".to_string())
			)
		}
	})
	.filter_map(Result::ok)
	.collect())
}

pub fn get_and_check_perspective(perspective: &Address) -> ZomeApiResult<app_definitions::Perspective>{
    let entry = hdk::api::get_entry(perspective)?;
    match entry {
        Some(Entry::App(_, entry_value)) => {
            let perspective_entry = app_definitions::Perspective::try_from(&entry_value).map_err(|_err| ZomeApiError::from("Specified perspective address is not of type Perspective".to_string()))?; //will return error here if cannot ser entry to group
            Ok(perspective_entry)
        },
        Some(_) => Err(ZomeApiError::from("Context address was not an app entry".to_string())),
        None => Err(ZomeApiError::from("No perspective entry at specified address".to_string()))
    }
}

///Sorts vector of times into ordered vector from year -> hour
pub fn sort_time_vector(times: Vec<&str>) -> Vec<&str> {
    let search_times = vec!["time:y>", "time:m>", "time:d>", "time:h>"];
    let mut times_out = vec![];
    let time_types = times.clone().into_iter().map(|time| time.split("<").collect::<Vec<_>>()[1]).collect::<Vec<_>>();
    for search_time in &search_times{
        match time_types.iter().position(|time_type| time_type == search_time){
            Some(index) => {
                times_out.push(times[index].clone())
            },
            None => times_out.push("*")
        }; 
    };
    times_out
}

pub fn has_unique_elements<T>(iter: T) -> bool
where
    T: IntoIterator,
    T::Item: Eq + Hash,
{
    let mut uniq = HashSet::new();
    iter.into_iter().all(move |x| uniq.insert(x))
}

pub fn get_entries_timestamp(entry: &Address) -> ZomeApiResult<HashMap<&'static str, String>>{
    let mut out = HashMap::new();
    match hdk::get_entry_result(entry, GetEntryOptions {headers: true, ..Default::default()},)?.result {
        GetEntryResultType::Single(result) => {
            let iso_timestamp = serde_json::to_string(&result.headers[0].timestamp()).map_err(|err| ZomeApiError::from(err.to_string()))?; //TODO: ensure this is the actual header we want to use
            hdk::debug(format!("Got iso timestamp: {:?}", iso_timestamp))?;
            out.insert("year", iso_timestamp[1..5].to_lowercase());
            out.insert("month", iso_timestamp[6..8].to_lowercase());
            out.insert("day", iso_timestamp[9..11].to_lowercase());
            out.insert("hour", iso_timestamp[12..14].to_lowercase());
        },  
        GetEntryResultType::All(_entry_history) => {
            return Err(ZomeApiError::from("EntryResultType not of enum variant Single".to_string()))
        }
    };
    Ok(out)
}

///Checks if username_address can access context at given context address
///Returns privacy of context or err if cannot access the given context
pub fn run_context_auth(context: &Address, username_address: &Address) -> ZomeApiResult<Option<ContextAuthResult>>{
    match hdk::utils::get_as_type::<app_definitions::Collection>(context.clone()) {
        Ok(context_entry) => {
            hdk::debug("Context type collection, running auth")?;
            //check that current user making post is owner of den they are trying to post into
            if collection::is_collection_owner(context.clone(), username_address.clone())? == false{
                Err(ZomeApiError::from("You are attempting to access a collection which you do not own".to_string()))
            } else {
                Ok(Some(ContextAuthResult::Collection(context_entry)))
            }
        },
        Err(_err) => {
            hdk::debug("Context type group, running auth")?;
            let context_entry = hdk::utils::get_as_type::<app_definitions::Group>(context.clone()).ok();
            match context_entry{
                Some(context_entry) => {
                    if context_entry.privacy != app_definitions::Privacy::Public {
                        if (group::is_group_owner(context.clone(), username_address.clone())? == false) & (group::is_group_member(context.clone(), username_address.clone())? == false){
                            return Err(ZomeApiError::from("You are attempting to access a group you are not permitted to interact with".to_string()))
                        };
                    };
                    Ok(Some(ContextAuthResult::Group(context_entry)))
                },
                None => Ok(None)
            }
        }
    }
}

pub fn get_expression_attributes(expression_data: EntryAndAddress<app_definitions::ExpressionPost>, fetch_sub_expressions: bool) -> ZomeApiResult<ExpressionData> {
    let user = get_links_and_load_type::<app_definitions::UserName>(&expression_data.address, LinkMatch::Exactly("auth"), LinkMatch::Exactly("owner"))?;
    let profile = get_links_and_load_type::<app_definitions::User>(&user[0].address, LinkMatch::Exactly("profile"), LinkMatch::Any)?;
    let timestamp = get_entries_timestamp(&expression_data.address)?;
    let channels = get_links_and_load_type::<app_definitions::Attribute>(&expression_data.address, LinkMatch::Exactly("channels"), LinkMatch::Any)?;
    let mut sub_expressions = vec![];
    if fetch_sub_expressions == true {
        hdk::debug("Getting sub expressions")?;
        sub_expressions = get_links_and_load_type::<app_definitions::ExpressionPost>(&expression_data.address, LinkMatch::Exactly("sub_expression"), LinkMatch::Any)?
                                .into_iter().map(|sub_expression| get_expression_attributes(sub_expression, false)).collect::<Result<Vec<_>,_>>()?;
    }
    let resonations = get_links_and_load_type::<app_definitions::UserName>(&expression_data.address, LinkMatch::Exactly("resonation"), LinkMatch::Any)?;
    Ok(ExpressionData{expression: expression_data, sub_expressions: sub_expressions, author_username: user[0].clone(), author_profile: profile[0].clone(), 
                        resonations: resonations, timestamp: format!("{}-{}-{}-{}", timestamp["year"], timestamp["month"], timestamp["day"], timestamp["hour"]),
                        channels: channels})
}